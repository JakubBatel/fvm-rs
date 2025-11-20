use anyhow::{Context, Result};
use clap::Args;
use dialoguer::{theme::ColorfulTheme, Select};
use std::env;
use tracing::info;

use crate::{config_manager, gitignore_manager, ide_manager, sdk_manager};

#[derive(Debug, Clone, Args)]
pub struct UseArgs {
    /// Flutter version to use (e.g., "3.24.0", "stable"), or flavor name to switch to
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

    /// Pin this version to a specific project flavor/environment
    #[arg(long, visible_alias = "env", value_name = "FLAVOR_NAME")]
    flavor: Option<String>,
}

pub async fn run(args: UseArgs) -> Result<()> {
    // Get current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;

    // Get version from args or interactive selector
    let version_input = if let Some(v) = args.version {
        v
    } else {
        select_version_interactively().await?
    };

    // Check if version_input is actually a flavor name in the project config
    let (resolved_version, is_flavor_switch) = resolve_version_or_flavor(&current_dir, &version_input).await?;

    // Validation: Cannot use --flavor flag when switching to a flavor
    if is_flavor_switch && args.flavor.is_some() {
        anyhow::bail!(
            "Cannot use --flavor option when switching to a flavor. \
            You provided flavor name '{}' as the version. \
            Use 'fvm-rs use <version> --flavor {}' to update the flavor instead.",
            version_input,
            version_input
        );
    }

    // Use the resolved version for installation and config
    let version_to_install = resolved_version.clone();

    if is_flavor_switch {
        println!("Using Flutter SDK from flavor: \"{}\" which is \"{}\"", version_input, resolved_version);
        info!("Switching to flavor {} (version {})", version_input, resolved_version);
    } else {
        info!("Switching project to Flutter SDK version: {}", version_to_install);
    }

    if args.skip_setup {
        // TODO: Implement skip_setup functionality
        tracing::warn!("--skip-setup flag is not yet fully implemented");
    }

    if args.force {
        // TODO: Implement force flag to bypass Flutter project validation
        tracing::debug!("Force flag enabled, bypassing validations");
    }

    // Ensure the version is installed first
    sdk_manager::ensure_installed(&version_to_install).await?;

    info!("Creating FVM configuration in: {}", current_dir.display());

    // Update config based on whether we're using --flavor flag
    if let Some(flavor_name) = &args.flavor {
        // Pin version to a flavor
        config_manager::update_project_config(
            &current_dir,
            Some(&version_to_install),
            Some((flavor_name, &version_to_install)),
        )
        .await
        .context("Failed to update project configuration with flavor")?;

        println!("✓ Project now uses Flutter SDK: {} on [{}] flavor", version_to_install, flavor_name);
        info!("Successfully pinned version {} to flavor {}", version_to_install, flavor_name);
    } else {
        // Regular version switch (may be from flavor resolution)
        // Use update_project_config to preserve existing flavors
        config_manager::update_project_config(
            &current_dir,
            Some(&version_to_install),
            None, // Don't add/update any flavor, just preserve existing ones
        )
        .await
        .context("Failed to write project configuration")?;

        if is_flavor_switch {
            println!("✓ Project now uses Flutter SDK version: {} (from [{}] flavor)", version_to_install, version_input);
        } else {
            println!("✓ Project now uses Flutter SDK version: {}", version_to_install);
        }
        info!("Successfully configured project to use Flutter SDK {}", version_to_install);
    }

    println!("  Config saved to .fvmrc and .fvm/fvm_config.json");

    // Update .fvm/.gitignore to ignore flutter_sdk symlink
    gitignore_manager::update_fvm_gitignore(&current_dir)
        .await
        .context("Failed to update .fvm/.gitignore")?;

    // Read global config to check IDE integration settings
    let global_config = config_manager::GlobalConfig::read().await?;

    // Update VS Code settings if enabled (default: true)
    if global_config.update_vscode_settings.unwrap_or(true) {
        info!("Updating VS Code settings");
        match ide_manager::update_vscode_settings(&current_dir).await {
            Ok(()) => {
                tracing::debug!("VS Code settings updated successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to update VS Code settings: {}", e);
            }
        }

        // Also update workspace files if present
        match ide_manager::update_vscode_workspace(&current_dir).await {
            Ok(()) => {
                tracing::debug!("VS Code workspace files updated successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to update VS Code workspace files: {}", e);
            }
        }
    }

    // Update IntelliJ/Android Studio settings if enabled (default: true)
    if global_config.update_vscode_settings.unwrap_or(true) {
        info!("Updating IntelliJ/Android Studio settings");
        match ide_manager::update_intellij_settings(&current_dir).await {
            Ok(()) => {
                tracing::debug!("IntelliJ settings updated successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to update IntelliJ settings: {}", e);
            }
        }
    }

    // Update project .gitignore if enabled (default: false for backward compatibility)
    if global_config.update_gitignore.unwrap_or(false) {
        info!("Updating project .gitignore");
        match gitignore_manager::update_project_gitignore(&current_dir).await {
            Ok(()) => {
                tracing::debug!("Project .gitignore updated successfully");
            }
            Err(e) => {
                tracing::warn!("Failed to update project .gitignore: {}", e);
            }
        }
    }

    // Run flutter pub get unless skipped
    if !args.skip_pub_get {
        info!("Running flutter pub get");
        println!("\nRunning flutter pub get...");

        match run_flutter_pub_get(&current_dir, &version_to_install).await {
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

/// Resolve whether the input is a version or a flavor name
///
/// Returns (resolved_version, is_flavor_switch).
/// If input is a flavor name, returns the pinned version and true.
/// Otherwise, returns the input as-is and false.
async fn resolve_version_or_flavor(
    project_root: &std::path::Path,
    version_input: &str,
) -> Result<(String, bool)> {
    // Try to read existing project config
    if let Some(config) = config_manager::read_project_config(project_root).await? {
        // Check if version_input matches a flavor name
        if let Some(flavors) = &config.flavors {
            if let Some(flavor_version) = flavors.get(version_input) {
                // It's a flavor name! Resolve to its version
                return Ok((flavor_version.clone(), true));
            }
        }
    }

    // Not a flavor, return as-is
    Ok((version_input.to_string(), false))
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
