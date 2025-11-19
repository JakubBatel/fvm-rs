use anyhow::{bail, Result};
use clap::Args;
use tracing::{debug, info};

use crate::{config_manager, sdk_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct ExecArgs {
    /// Command and arguments to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    command_args: Vec<String>,
}

pub async fn run(args: ExecArgs) -> Result<i32> {
    // Validate that at least a command is provided
    if args.command_args.is_empty() {
        eprintln!("✗ No command provided");
        eprintln!("  Usage: fvm-rs exec <command> [args...]");
        bail!("No command was provided to be executed");
    }

    let command = &args.command_args[0];
    let command_args = &args.command_args[1..];

    info!("Executing command: {} {}", command, command_args.join(" "));

    // Try to resolve version: project -> global -> system PATH
    let project_version = config_manager::get_project_flutter_version().await?;
    let global_version = config_manager::get_global_flutter_version().await?;

    // Determine which version to use
    if let Some(version) = project_version {
        debug!("Using project version: {}", version);
        info!("Running with project version: {}", version);

        // Ensure version is installed (auto-install if configured but not cached)
        sdk_manager::ensure_installed(&version).await?;

        // Get the Flutter installation path
        let flutter_path = utils::flutter_version_dir(&version)?;

        // Execute with modified PATH
        let exit_code = utils::execute_with_flutter_path(command, command_args, &flutter_path)?;
        Ok(exit_code)
    } else if let Some(version) = global_version {
        debug!("Using global version: {}", version);
        info!("Running with global version: {}", version);

        // Ensure version is installed (auto-install if configured but not cached)
        sdk_manager::ensure_installed(&version).await?;

        // Get the Flutter installation path
        let flutter_path = utils::flutter_version_dir(&version)?;

        // Execute with modified PATH
        let exit_code = utils::execute_with_flutter_path(command, command_args, &flutter_path)?;
        Ok(exit_code)
    } else {
        debug!("No FVM version configured, using system PATH");
        info!("Running with system PATH");

        // Fallback to system PATH
        let exit_code = utils::execute_with_system_path(command, command_args)?;
        Ok(exit_code)
    }
}
