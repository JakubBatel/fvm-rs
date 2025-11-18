use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};
use std::env;
use tracing::info;

use crate::{config_manager, gitignore_manager, sdk_manager};

#[derive(Debug, Clone, Args)]
pub struct UseArgs {
    /// Flutter version to use (e.g., "3.24.0", "stable")
    version: Option<String>,

    /// Skip running "flutter pub get" after switching SDK versions
    #[arg(long)]
    skip_pub_get: bool,

    /// Skip downloading SDK dependencies (engine) after switching
    #[arg(long, short = 's')]
    skip_setup: bool,

    /// Bypass Flutter project validation checks
    #[arg(long, short = 'f')]
    force: bool,
}

pub async fn run(args: UseArgs) -> Result<()> {
    // Get version from args or interactive selector
    let version = if let Some(v) = args.version {
        v
    } else {
        select_version_interactively().await?
    };

    info!("Switching project to Flutter SDK version: {}", version);

    if args.skip_setup {
        // TODO: Implement skip_setup functionality
        tracing::warn!("--skip-setup flag is not yet fully implemented");
    }

    if args.force {
        // TODO: Implement force flag to bypass Flutter project validation
        tracing::debug!("Force flag enabled, bypassing validations");
    }

    // Ensure the version is installed first
    sdk_manager::ensure_installed(&version).await?;

    // Get current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    info!("Creating FVM configuration in: {}", current_dir.display());

    // Write both .fvmrc and .fvm/fvm_config.json
    config_manager::write_project_config(&current_dir, &version)
        .await
        .context("Failed to write project configuration")?;

    // Update .fvm/.gitignore to ignore flutter_sdk symlink
    gitignore_manager::update_fvm_gitignore(&current_dir)
        .await
        .context("Failed to update .fvm/.gitignore")?;

    println!("✓ Project now uses Flutter SDK version: {}", version);
    println!("  Config saved to .fvmrc and .fvm/fvm_config.json");
    info!("Successfully configured project to use Flutter SDK {}", version);

    // Run flutter pub get unless skipped
    if !args.skip_pub_get {
        info!("Running flutter pub get");
        println!("\nRunning flutter pub get...");

        match run_flutter_pub_get(&current_dir, &version).await {
            Ok(()) => {
                println!("✓ Dependencies resolved");
            }
            Err(e) => {
                tracing::warn!("Failed to run pub get: {}", e);
                println!("⚠ Warning: Failed to run pub get: {}", e);
                println!("  You may need to run 'flutter pub get' manually");
            }
        }
    }

    Ok(())
}

async fn select_version_interactively() -> Result<String> {
    info!("Selecting Flutter version interactively");

    // Get list of installed versions
    let versions = sdk_manager::list_installed_versions().await?;

    if versions.is_empty() {
        anyhow::bail!(
            "No Flutter versions installed.\nRun 'fvm-rs install <version>' to install one first."
        );
    }

    // Show selection menu
    let selection = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a Flutter version to use for this project")
        .items(&versions)
        .default(0)
        .interact()
        .context("Failed to get user selection")?;

    Ok(versions[selection].clone())
}

async fn run_flutter_pub_get(project_dir: &std::path::Path, version: &str) -> Result<()> {
    use crate::utils;
    use std::process::Command;

    // Get the Flutter installation path for this version
    let flutter_path = utils::flutter_version_dir(version)?;

    if !flutter_path.exists() {
        anyhow::bail!("Flutter version {} is not installed", version);
    }

    // Construct path to flutter executable
    let flutter_bin = flutter_path.join("bin").join(if cfg!(windows) {
        "flutter.bat"
    } else {
        "flutter"
    });

    // Run flutter pub get in the project directory
    let output = Command::new(&flutter_bin)
        .args(&["pub", "get"])
        .current_dir(project_dir)
        .output()
        .context("Failed to execute flutter pub get")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pub get failed: {}", stderr);
    }

    Ok(())
}
