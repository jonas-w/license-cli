use clap::Parser;
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
        .ok_or_else(|| AppError::LicenseNotFound(spdx_identifier))?;

    let name = license["name"]
        .as_str()
        .ok_or(AppError::MalformedLicenseData)?;
    let details_url = license["detailsUrl"]
        .as_str()
        .ok_or(AppError::MalformedLicenseData)?;

    println!("License found: {}", name);
    println!("Details URL: {}", details_url);

    Ok(())
}
