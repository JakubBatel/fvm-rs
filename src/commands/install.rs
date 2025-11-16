use anyhow::Result;
use clap::Args;

use crate::sdk_manager;

#[derive(Debug, Clone, Args)]
pub struct InstallArgs {
    version: String,
}

pub async fn run(args: InstallArgs) -> Result<()> {
    sdk_manager::ensure_installed(&args.version).await?;
    println!("Flutter SDK {} has been installed successfully", args.version);
    return Ok(());
}
