use crate::sdk_manager;
use anyhow::Result;
use tracing::info;

pub async fn run() -> Result<()> {
    info!("Listing installed Flutter SDK versions");
    let versions = sdk_manager::list_installed_versions().await?;

    info!("Found {} installed version(s)", versions.len());
    for version in versions {
        println!("{}", version);
    }

    return Ok(());
}
