use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Confirm};
use tracing::{debug, info};

use crate::utils;

#[derive(Debug, Clone, Args)]
pub struct DestroyArgs {
    /// Bypass confirmation prompt
    #[arg(short, long)]
    pub force: bool,
}

pub async fn run(args: DestroyArgs) -> Result<()> {
    let fvm_dir = utils::fvm_rs_root_dir()?;

    info!("Destroy command invoked");
    debug!("FVM directory: {}", fvm_dir.display());

    // Check if directory exists
    if !fvm_dir.exists() {
        println!("FVM directory does not exist: {}", fvm_dir.display());
        return Ok(());
    }

    // Get confirmation unless --force is used
    let proceed = if args.force {
        debug!("Force flag set, bypassing confirmation");
        true
    } else {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Are you sure you want to destroy the FVM cache directory and references?\n\
                 This action cannot be undone. Do you want to proceed?"
            )
            .default(false)
            .interact()
            .context("Failed to get confirmation")?
    };

    if !proceed {
        println!("Operation cancelled");
        return Ok(());
    }

    // Remove the entire FVM directory
    info!("Removing FVM directory: {}", fvm_dir.display());
    tokio::fs::remove_dir_all(&fvm_dir)
        .await
        .context("Failed to remove FVM directory")?;

    println!("✓ FVM directory {} has been deleted", fvm_dir.display());
    debug!("Destroy operation completed successfully");

    Ok(())
}
