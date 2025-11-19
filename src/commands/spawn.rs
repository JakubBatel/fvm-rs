use anyhow::{bail, Result};
use clap::Args;
use tracing::{debug, info};

use crate::{sdk_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct SpawnArgs {
    /// Flutter SDK version to use
    pub version: Option<String>,

    /// Flutter command and arguments to execute
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    flutter_args: Vec<String>,
}

pub async fn run(args: SpawnArgs) -> Result<i32> {
    // Validate that a version is provided
    let version = args.version.ok_or_else(|| {
        eprintln!("✗ No version provided");
        eprintln!("  Usage: fvm-rs spawn <version> <flutter_command> [args...]");
        anyhow::anyhow!("Need to provide a version to spawn a Flutter command")
    })?;

    debug!("Spawning Flutter command with version: {}", version);
    info!("Spawning version \"{}\"...", version);

    // Ensure version is installed (auto-install if not present)
    sdk_manager::ensure_installed(&version).await?;

    // Get the Flutter installation path
    let flutter_path = utils::flutter_version_dir(&version)?;

    // Check if version is installed
    if !flutter_path.exists() {
        eprintln!("✗ Flutter version {} is not installed", version);
        bail!("Flutter version {} not found", version);
    }

    debug!("Using Flutter at: {}", flutter_path.display());

    // Execute flutter command with modified PATH
    let exit_code = utils::execute_with_flutter_path("flutter", &args.flutter_args, &flutter_path)?;
    Ok(exit_code)
}
