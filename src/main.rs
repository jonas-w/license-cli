use clap::Parser;
use reqwest::blocking::Client;
use serde_json::Value;
use std::error::Error;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// The SPDX identifier of the license
    spdx_identifier: String,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let spdx_identifier = args.spdx_identifier;

    let client = Client::new();
    let licenses_json: Value = client
        .get("https://spdx.org/licenses/licenses.json")
        .send()?
        .json()?;

    let licenses = licenses_json["licenses"].as_array().unwrap();
    let license = licenses
        .iter()
        .find(|&license| license["licenseId"].as_str().unwrap() == spdx_identifier);

    match license {
        Some(license_data) => {
            println!("License found: {}", license_data["name"].as_str().unwrap());
            println!(
                "Details URL: {}",
                license_data["detailsUrl"].as_str().unwrap()
            );
        }
        None => println!("License not found for SPDX identifier: {}", spdx_identifier),
    }

    Ok(())
}
