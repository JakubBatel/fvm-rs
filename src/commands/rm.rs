use crate::sdk_manager;
use anyhow::Result;
use clap::Args;

#[derive(Debug, Clone, Args)]
pub struct RmArgs {
    version: String,
}

pub async fn run(args: RmArgs) -> Result<()> {
    sdk_manager::uninstall(&args.version).await?;
    return Ok(());
}
