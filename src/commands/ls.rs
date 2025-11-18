use crate::sdk_manager;
use anyhow::Result;
use tracing::info;

pub async fn run() -> Result<()> {
    info!("Listing installed Flutter SDK versions");
    let versions = sdk_manager::list_installed_versions().await?;
    let global_version = sdk_manager::get_global_version().await?;

    info!("Found {} installed version(s)", versions.len());

    for version in versions {
        // Add indicator for global version
        if let Some(ref global) = global_version {
            if global == &version {
                println!("\u{25cf} {}", version);
                continue;
            }
        }
        println!("  {}", version);
    }

    return Ok(());
}
