use clap::Parser;
use colored::*;
use reqwest::blocking::Client;
use serde_json::Value;
use thiserror::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The SPDX identifier of the license
    spdx_identifier: String,
}

#[derive(Error, Debug)]
enum AppError {
    #[error("HTTP request failed: {0}")]
    RequestFailed(#[from] reqwest::Error),
    #[error("License data is missing or malformed")]
    MalformedLicenseData,
    #[error("License not found for SPDX identifier: {0}")]
    LicenseNotFound(String),
}

fn fetch_license_details(url: &str) -> Result<Value, AppError> {
    let client = Client::new();
    client
        .get(url)
        .send()
        .map_err(AppError::RequestFailed)?
        .json()
        .map_err(AppError::RequestFailed)
}

fn main() -> Result<(), AppError> {
    let args = Args::parse();
    let spdx_identifier = args.spdx_identifier;

    let client = Client::new();
    let licenses_json: Value = client
        .get("https://spdx.org/licenses/licenses.json")
        .send()
        .map_err(AppError::RequestFailed)?
        .json()
        .map_err(AppError::RequestFailed)?;

    let licenses = licenses_json["licenses"]
        .as_array()
        .ok_or(AppError::MalformedLicenseData)?;

    let license = licenses
        .iter()
        .find(|&license| license["licenseId"].as_str() == Some(&spdx_identifier))
        .ok_or_else(|| AppError::LicenseNotFound(spdx_identifier.clone()))?;

    let details_url = license["detailsUrl"]
        .as_str()
        .ok_or(AppError::MalformedLicenseData)?;

    println!("Fetching license details...");
    let license_details = fetch_license_details(details_url)?;

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

    println!(
        "\n{}",
        "License Text Preview (first 200 characters):"
            .green()
            .bold()
    );
    if let Some(text) = license_details["licenseText"].as_str() {
        println!("{}", text.chars().take(200).collect::<String>().italic());
        println!("{}", "...".bright_black());
    } else {
        println!("{}", "License text not available".red());
    }

    Ok(())
}
