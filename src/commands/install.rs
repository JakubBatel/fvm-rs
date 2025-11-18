use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};
use tracing::info;

use crate::sdk_manager;

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    /// Flutter version to install (e.g., "3.24.0", "stable")
    version: Option<String>,

    /// Skip downloading SDK dependencies (engine) after install
    #[arg(long)]
    skip_setup: bool,
}

pub async fn run(args: InstallArgs) -> Result<()> {
    // Get version from args or interactive selector
    let version = if let Some(v) = args.version {
        v
    } else {
        select_version_interactively().await?
    };

    info!("Starting installation of Flutter SDK {}", version);

    if args.skip_setup {
        // TODO: Implement skip_setup functionality
        // For now, we always install the engine as it's required for Flutter to function
        tracing::warn!("--skip-setup flag is not yet fully implemented");
    }

    println!("Installing Flutter SDK {}...", version);
    sdk_manager::ensure_installed(&version).await?;
    println!("✓ Flutter SDK {} has been installed successfully", version);
    info!("Successfully installed Flutter SDK {}", version);
    return Ok(());
}

async fn select_version_interactively() -> Result<String> {
    info!("Selecting Flutter version interactively");
    println!("Fetching available Flutter releases...");

    // Fetch available releases
    let releases = sdk_manager::list_available_versions().await
        .context("Failed to fetch available Flutter releases")?;

    // Create list of options: channels first, then recent releases
    let mut options = vec![
        "stable (latest stable release)".to_string(),
        "beta (latest beta release)".to_string(),
        "dev (latest dev release)".to_string(),
        "master (bleeding edge)".to_string(),
    ];

    // Add separator
    options.push("──────────────────────────────".to_string());

    // Add recent stable releases (limit to 10)
    for release in releases.releases.iter()
        .filter(|r| r.channel == "stable")
        .take(10)
    {
        options.push(format!("{} (stable)", release.version));
    }

    // Show selection menu
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Flutter version to install")
        .items(&options)
        .default(0)
        .interact()
        .context("Failed to get user selection")?;

    // Extract version from selection
    let selected = &options[selection];

    if selection < 4 {
        // It's a channel
        let channel = selected.split_whitespace().next().unwrap();
        Ok(channel.to_string())
    } else if selection == 4 {
        // It's the separator, shouldn't happen
        anyhow::bail!("Invalid selection")
    } else {
        // It's a version number
        let version = selected.split_whitespace().next().unwrap();
        Ok(version.to_string())
    }
}
