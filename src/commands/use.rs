use anyhow::{Context, Result};
use clap::Args;
use std::env;
use tracing::info;

use crate::{config_manager, gitignore_manager, sdk_manager};

#[derive(Debug, Clone, Args)]
pub struct UseArgs {
    version: String,
}

pub async fn run(args: UseArgs) -> Result<()> {
    info!("Switching project to Flutter SDK version: {}", args.version);

    // Ensure the version is installed first
    sdk_manager::ensure_installed(&args.version).await?;

    // Get current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    info!("Creating FVM configuration in: {}", current_dir.display());

    // Write both .fvmrc and .fvm/fvm_config.json
    config_manager::write_project_config(&current_dir, &args.version)
        .await
        .context("Failed to write project configuration")?;

    // Update .fvm/.gitignore to ignore flutter_sdk symlink
    gitignore_manager::update_fvm_gitignore(&current_dir)
        .await
        .context("Failed to update .fvm/.gitignore")?;

    println!("✓ Project now uses Flutter SDK version: {}", args.version);
    println!("  Config saved to .fvmrc and .fvm/fvm_config.json");
    info!("Successfully configured project to use Flutter SDK {}", args.version);

    Ok(())
}
