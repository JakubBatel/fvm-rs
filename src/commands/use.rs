use anyhow::Result;
use clap::Args;

use crate::sdk_manager;

#[derive(Debug, Clone, Args)]
pub struct UseArgs {
    version: Option<String>,
}

pub async fn run(args: UseArgs) -> Result<()> {
    let version = match args.version {
        Some(version) => version,
        None => todo!(),
    };

    sdk_manager::ensure_installed(&version).await?;
    // TODO update the current version in the config

    return Ok(());
}
