use anyhow::{Context, Result};
use clap::Args;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::fs;

use crate::sdk_manager;

#[derive(Debug, Clone, Args)]
pub struct UseArgs {
    version: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct FvmConfig {
    #[serde(rename = "flutterSdkVersion")]
    flutter_sdk_version: String,
}

pub async fn run(args: UseArgs) -> Result<()> {
    // Ensure the version is installed first
    sdk_manager::ensure_installed(&args.version).await?;

    // Get current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    let fvm_dir = current_dir.join(".fvm");
    let config_path = fvm_dir.join("fvm_config.json");

    // Create .fvm directory if it doesn't exist
    fs::create_dir_all(&fvm_dir)
        .await
        .context("Failed to create .fvm directory")?;

    // Create the config object
    let config = FvmConfig {
        flutter_sdk_version: args.version.clone(),
    };

    // Write the config file
    let config_json = serde_json::to_string_pretty(&config)
        .context("Failed to serialize config")?;

    fs::write(&config_path, config_json)
        .await
        .context("Failed to write fvm_config.json")?;

    println!("Project now uses Flutter SDK version: {}", args.version);
    println!("Config saved to: {}", config_path.display());

    Ok(())
}
