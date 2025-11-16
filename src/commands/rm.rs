use crate::sdk_manager;
use anyhow::{Result, bail};
use clap::Args;
use std::io::{self, Write};

#[derive(Debug, Clone, Args)]
pub struct RmArgs {
    /// Flutter version to remove (e.g., 3.24.0)
    #[arg(required_unless_present = "all")]
    version: Option<String>,

    /// Remove all installed versions
    #[arg(short, long)]
    all: bool,

    /// Skip engine cleanup (faster, but may leave unused engines)
    #[arg(long)]
    skip_engine_cleanup: bool,
}

pub async fn run(args: RmArgs) -> Result<()> {
    // Validate arguments
    if !args.all && args.version.is_none() {
        bail!("Either specify a version or use --all flag");
    }

    if args.all && args.version.is_some() {
        bail!("Cannot specify both a version and --all flag");
    }

    // Handle --all flag
    if args.all {
        // Get confirmation from user
        print!("Are you sure you want to remove all installed Flutter versions? (y/N): ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if !input.trim().eq_ignore_ascii_case("y") && !input.trim().eq_ignore_ascii_case("yes") {
            println!("Cancelled.");
            return Ok(());
        }

        let versions = sdk_manager::list_installed_versions().await?;

        if versions.is_empty() {
            println!("No Flutter versions installed.");
            return Ok(());
        }

        println!("Removing {} version(s)...", versions.len());

        for version in versions {
            println!("Removing Flutter {}...", version);
            match sdk_manager::uninstall(&version).await {
                Ok(Some(hash)) => {
                    println!("✓ Removed Flutter {} (engine: {})", version, hash);
                }
                Ok(None) => {
                    println!("✓ Removed Flutter {} (no engine info)", version);
                }
                Err(e) => {
                    eprintln!("✗ Failed to remove Flutter {}: {}", version, e);
                }
            }
        }

        // Clean up engines unless skipped
        if !args.skip_engine_cleanup {
            println!("\nCleaning up unused engines...");
            match sdk_manager::cleanup_unused_engines().await {
                Ok(result) => {
                    for hash in &result.removed_engines {
                        println!("✓ Removed unused engine: {}", hash);
                    }
                    for (hash, error) in &result.failed_removals {
                        eprintln!("✗ Failed to remove engine {}: {}", hash, error);
                    }
                    if result.removed_engines.is_empty() && result.failed_removals.is_empty() {
                        println!("No unused engines to remove");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Engine cleanup failed: {}", e);
                }
            }
        }

        println!("\nAll versions removed successfully!");
        return Ok(());
    }

    // Handle single version removal
    let version = args.version.as_ref().unwrap();

    // Check if version exists
    let installed = sdk_manager::list_installed_versions().await?;
    if !installed.contains(version) {
        bail!("Flutter version {} is not installed", version);
    }

    println!("Removing Flutter {}...", version);

    match sdk_manager::uninstall(version).await {
        Ok(Some(hash)) => {
            println!("✓ Removed Flutter {} (engine: {})", version, hash);

            // Clean up engines unless skipped
            if !args.skip_engine_cleanup {
                println!("Checking for unused engines...");
                match sdk_manager::cleanup_unused_engines().await {
                    Ok(result) => {
                        for hash in &result.removed_engines {
                            println!("✓ Removed unused engine: {}", hash);
                        }
                        for (hash, error) in &result.failed_removals {
                            eprintln!("✗ Failed to remove engine {}: {}", hash, error);
                        }
                    }
                    Err(e) => {
                        eprintln!("Warning: Engine cleanup failed: {}", e);
                    }
                }
            }
        }
        Ok(None) => {
            println!("✓ Removed Flutter {} (no engine info)", version);
        }
        Err(e) => {
            bail!("Failed to remove Flutter {}: {}", version, e);
        }
    }

    return Ok(());
}
