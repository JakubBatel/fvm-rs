use crate::sdk_manager;
use anyhow::Result;

pub async fn run() -> Result<()> {
    let versions = sdk_manager::list_installed_versions().await?;

    for version in versions {
        println!("{}", version);
    }

    return Ok(());
}
