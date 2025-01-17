use clap::Parser;
use colored::*;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache, HttpCacheOptions};
use platform_dirs::AppDirs;
use reqwest::Client;
use reqwest_middleware::{self, ClientWithMiddleware};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use std::thread::spawn;
use thiserror::Error;

use nucleo_picker::Picker;
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// The SPDX identifier of the license
    spdx_identifier: Option<String>,

    /// Output full license text to file
    #[arg(short, long, value_name = "FILE")]
    output: Option<PathBuf>,
    #[arg(short, long, default_value_t = false)]
    full_text: bool,
}
#[derive(Error, Debug)]
enum AppError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("HTTP request failed: {0}")]
    MiddleWareRequestFailed(#[from] reqwest_middleware::Error),
    #[error("License data is missing or malformed")]
    MalformedLicenseData,
    #[error("License not found for SPDX identifier: {0}")]
    LicenseNotFound(String),
    #[error("Failed to write to file: {0}")]
    FileWriteError(#[from] std::io::Error),
}
fn fuzzy_find_license(licenses: Vec<Value>) -> Result<Option<Value>, AppError> {
    let mut picker = Picker::default();

    let injector = picker.injector();
    spawn(move || {
        for license in licenses.into_iter() {
            injector.push(license, |e, cols| {
                cols[0] = format!(
                    "{} - {}",
                    e["licenseId"].as_str().unwrap_or("Malformed license"),
                    e["name"].as_str().unwrap_or("Malformed license")
                )
                .into();
            });
        }
    });

    match picker.pick() {
        Ok(picked) => {
            if let Some(maybe_spdx) = picked {
                Ok(Some(maybe_spdx.to_owned().to_owned()))
            } else {
                Ok(None)
            }
        }
        Err(_) => Err(AppError::LicenseNotFound(
            "No License selected.".to_string(),
        )),
    }
}
async fn fetch_license_details(
    client: &ClientWithMiddleware,
    url: &str,
) -> Result<Value, AppError> {
    client
        .get(url)
        .send()
        .await
        .map_err(AppError::MiddleWareRequestFailed)?
        .json()
        .await
        .map_err(AppError::RequestFailed)
}

fn display_preview(license_details: &Value, full_text: bool) {
    println!("\n{}", "License Preview:".green().bold());
    println!("{}", "----------------".green());

    println!(
        "{}: {}",
        "Name".cyan().bold(),
        license_details["name"].as_str().unwrap_or("N/A").white()
    );
    println!(
        "{}: {}",
        "SPDX ID".cyan().bold(),
        license_details["licenseId"]
            .as_str()
            .unwrap_or("N/A")
            .white()
    );
    println!(
        "{}: {}",
        "Is OSI Approved".cyan().bold(),
        if license_details["isOsiApproved"].as_bool().unwrap_or(false) {
            "Yes".green()
        } else {
            "No".red()
        }
    );

    if let Some(deprecated) = license_details["isDeprecatedLicenseId"].as_bool() {
        println!(
            "{}: {}",
            "Deprecated".cyan().bold(),
            if deprecated {
                "Yes".red()
            } else {
                "No".green()
            }
        );
    }

    if let Some(see_also) = license_details["seeAlso"].as_array() {
        println!("{}", "See Also:".cyan().bold());
        for url in see_also {
            println!("  - {}", url.as_str().unwrap_or("N/A").blue().underline());
        }
    }

    if let Some(text) = license_details["licenseText"].as_str() {
        println!();
        if full_text {
            println!("{}", "Full License Text".green().bold());

            println!("{}", text);
        } else {
            println!("{}", "License Text Preview (first 200 chars)".cyan().bold());
            println!("{}", text.chars().take(200).collect::<String>().italic());
            println!("{}", "...".bright_black());
        }
    } else {
        println!("{}", "License text not available".red());
    }
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let args = Cli::parse();

    let client = reqwest_middleware::ClientBuilder::new(Client::new())
        .with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: CACacheManager {
                path: AppDirs::new(Some("license-cli"), true)
                    .expect("Failed to get cache directory")
                    .cache_dir,
            },
            options: HttpCacheOptions::default(),
        }))
        .build();
    let licenses_json: Value = client
        .get("https://spdx.org/licenses/licenses.json")
        .send()
        .await
        .map_err(AppError::MiddleWareRequestFailed)?
        .json()
        .await
        .map_err(AppError::RequestFailed)?;
    let licenses = licenses_json["licenses"]
        .as_array()
        .ok_or(AppError::MalformedLicenseData)?;

    let license = if let Some(spdx_identifier) = args.spdx_identifier {
        licenses
            .iter()
            .find(|&license| license["licenseId"].as_str() == Some(&spdx_identifier))
            .ok_or_else(|| AppError::LicenseNotFound(spdx_identifier.clone()))?
            .to_owned()
    } else {
        fuzzy_find_license(licenses.to_owned())?.expect("No license selected")
    };

    let details_url = license["detailsUrl"]
        .as_str()
        .ok_or(AppError::MalformedLicenseData)?;

    println!("{}", "Fetching license details...".yellow());
    let license_details = fetch_license_details(&client, details_url).await?;

    display_preview(&license_details, args.full_text);

    if let Some(out) = args.output {
        if let Some(license_text) = license_details["licenseText"].as_str() {
            fs::write(&out, license_text)?;
            println!("\n{} {}", "License text written to:".green(), out.display());
        } else {
            println!("{}", "License text not available for writing to file".red());
        }
    }
    Ok(())
}
