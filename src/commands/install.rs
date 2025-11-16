use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::sdk_manager;

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    version: String,
}

pub async fn run(args: InstallArgs) -> Result<()> {
    info!("Starting installation of Flutter SDK {}", args.version);
    println!("Installing Flutter SDK {}...", args.version);
    sdk_manager::ensure_installed(&args.version).await?;
    println!("✓ Flutter SDK {} has been installed successfully", args.version);
    info!("Successfully installed Flutter SDK {}", args.version);
    return Ok(());
}
