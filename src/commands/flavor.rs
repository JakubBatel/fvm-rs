use anyhow::{Context, Result};
use clap::Args;
use tracing::info;

use crate::{config_manager, sdk_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct FlavorArgs {
    /// Flavor name to use (e.g., "production", "staging", "development")
    flavor_name: String,

    /// Flutter command and arguments to execute with the flavor's SDK version
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    flutter_args: Vec<String>,
}

pub async fn run(args: FlavorArgs) -> Result<()> {
    // Find project root (may be in a subdirectory)
    let project_root = config_manager::find_project_root()
        .await?
        .context("Not in an FVM project. Run 'fvm-rs use' to configure this project first.")?;

    info!("Using flavor '{}' from project at: {}", args.flavor_name, project_root.display());

    // Read project config
    let config = config_manager::read_project_config(&project_root)
        .await?
        .context("No FVM configuration found. Run 'fvm-rs use' to configure this project first.")?;

    // Get the version for this flavor
    let version = config
        .flavors
        .as_ref()
        .and_then(|flavors| flavors.get(&args.flavor_name))
        .context(format!(
            "Flavor '{}' is not defined in project configuration.\n\
            Available flavors: {}\n\n\
            Use 'fvm-rs use <version> --flavor {}' to define this flavor.",
            args.flavor_name,
            config
                .flavors
                .as_ref()
                .map(|f| {
                    if f.is_empty() {
                        "none".to_string()
                    } else {
                        f.keys().cloned().collect::<Vec<_>>().join(", ")
                    }
                })
                .unwrap_or_else(|| "none".to_string()),
            args.flavor_name
        ))?;

    info!("Flavor '{}' resolved to version: {}", args.flavor_name, version);
    println!("Running Flutter command with [{}] flavor (version: {})", args.flavor_name, version);

    // Ensure the version is installed
    sdk_manager::ensure_installed(version).await?;

    // Get the Flutter installation path
    let flutter_path = utils::flutter_version_dir(version)
        .context(format!("Failed to get Flutter path for version {}", version))?;

    if !flutter_path.exists() {
        anyhow::bail!("Flutter version {} is not installed at expected path: {}", version, flutter_path.display());
    }

    // Execute the Flutter command with this version
    let exit_code = utils::execute_with_flutter_path(
        "flutter",
        &args.flutter_args,
        &flutter_path,
    )
    .context("Failed to execute Flutter command")?;

    // Exit with the same code as the Flutter command
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}
