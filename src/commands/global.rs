use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};
use std::process::Command;
use tracing::info;

use crate::{sdk_manager, utils};

#[derive(Args, Debug, Clone)]
pub struct GlobalArgs {
    /// Flutter version to set as global (e.g., "3.24.0", "stable")
    version: Option<String>,

    /// Remove the global Flutter SDK version setting
    #[arg(short, long)]
    unlink: bool,

    /// Skip Flutter SDK validation checks
    #[arg(short, long)]
    force: bool,
}

pub async fn run(args: GlobalArgs) -> Result<()> {
    if args.unlink {
        return unlink_global().await;
    }

    let version = if let Some(v) = args.version {
        v
    } else {
        // Interactive mode: show menu of installed versions
        select_version_interactively().await?
    };

    set_global(&version, args.force).await
}

async fn set_global(version: &str, force: bool) -> Result<()> {
    info!("Setting global Flutter version to: {}", version);

    // Attempt to install the version if not already installed
    // (This mirrors FVM's behavior)
    let flutter_version_dir = utils::flutter_version_dir(version)?;
    if !flutter_version_dir.exists() {
        println!("Flutter version {} is not installed.", version);
        println!("Installing...");

        sdk_manager::ensure_installed(version).await
            .context("Failed to install Flutter version")?;
    }

    // Set the global version (creates symlink)
    sdk_manager::set_global_version(version).await
        .context("Failed to set global version")?;

    println!("✓ Flutter SDK: {} is now global", version);

    // Check PATH configuration
    if !force {
        check_path_configuration().await?;
    }

    Ok(())
}

async fn unlink_global() -> Result<()> {
    info!("Unlinking global Flutter version");

    let was_set = sdk_manager::unset_global_version().await
        .context("Failed to unlink global version")?;

    if was_set {
        println!("✓ Global version unlinked");
    } else {
        println!("No global version is set");
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
        .with_prompt("Select a Flutter version to set as global")
        .items(&versions)
        .default(0)
        .interact()
        .context("Failed to get user selection")?;

    Ok(versions[selection].clone())
}

async fn check_path_configuration() -> Result<()> {
    // Check where the `flutter` command currently points
    let which_output = Command::new("which")
        .arg("flutter")
        .output();

    if let Ok(output) = which_output {
        if output.status.success() {
            let current_path = String::from_utf8_lossy(&output.stdout).trim().to_string();

            // Expected path for global version
            let global_link = utils::get_global_link_path()?;
            let expected_bin = global_link.join("bin");
            let expected_flutter = expected_bin.join("flutter");

            // Check if current path matches expected
            if !current_path.starts_with(&expected_bin.to_string_lossy().to_string()) {
                println!("\n⚠️  Warning: Your configured \"flutter\" path may be incorrect");
                println!("   CURRENT:   {}", current_path);
                println!("   EXPECTED:  {}", expected_flutter.display());
                println!("\n   To fix this, add the following to your PATH:");
                println!("   export PATH=\"{}:$PATH\"", expected_bin.display());
                println!("\n   Or add it to your shell profile (~/.bashrc, ~/.zshrc, etc.)");
            }
        }
    }

    Ok(())
}
