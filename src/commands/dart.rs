use anyhow::Result;
use clap::Args;
use tracing::{debug, info};

use crate::{config_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct DartArgs {
    /// Arguments to pass to dart command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    args: Vec<String>,
}

pub async fn run(args: DartArgs) -> Result<i32> {
    info!("Running Dart command with FVM-managed version");

    // Try to resolve version: project -> global -> system PATH
    let project_version = config_manager::get_project_flutter_version().await?;
    let global_version = config_manager::get_global_flutter_version().await?;

    // Determine which version to use
    if let Some(version) = project_version {
        debug!("Using project version: {}", version);
        info!("Running Dart from project version: {}", version);

        // Get the Flutter installation path
        let flutter_path = utils::flutter_version_dir(&version)?;

        // Check if version is installed
        if !flutter_path.exists() {
            eprintln!("✗ Flutter version {} is not installed", version);
            eprintln!("  Run: fvm-rs install {}", version);
            anyhow::bail!("Flutter version {} not found", version);
        }

        // Execute with modified PATH
        let exit_code = utils::execute_with_flutter_path("dart", &args.args, &flutter_path)?;
        Ok(exit_code)
    } else if let Some(version) = global_version {
        debug!("Using global version: {}", version);
        info!("Running Dart from global version: {}", version);

        // Get the Flutter installation path
        let flutter_path = utils::flutter_version_dir(&version)?;

        // Check if version is installed
        if !flutter_path.exists() {
            eprintln!("✗ Flutter version {} is not installed", version);
            eprintln!("  Run: fvm-rs install {}", version);
            anyhow::bail!("Flutter version {} not found", version);
        }

        // Execute with modified PATH
        let exit_code = utils::execute_with_flutter_path("dart", &args.args, &flutter_path)?;
        Ok(exit_code)
    } else {
        debug!("No FVM version configured, using system Dart");
        info!("Running Dart from system PATH");

        // Fallback to system PATH
        let exit_code = utils::execute_with_system_path("dart", &args.args)?;
        Ok(exit_code)
    }
}
