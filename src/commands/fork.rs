use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use tracing::info;

use crate::config_manager::GlobalConfig;

#[derive(Debug, Clone, Args)]
pub struct ForkArgs {
    #[command(subcommand)]
    pub command: ForkCommands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ForkCommands {
    /// Add a new Flutter fork alias
    Add {
        /// Fork alias name (e.g., "mycompany")
        alias: String,
        /// Git repository URL (must end with .git)
        git_url: String,
    },
    /// Remove a Flutter fork alias
    Remove {
        /// Fork alias name to remove
        alias: String,
    },
    /// List all configured Flutter forks
    List,
}

pub async fn run(args: ForkArgs) -> Result<()> {
    match args.command {
        ForkCommands::Add { alias, git_url } => add_fork(&alias, &git_url).await,
        ForkCommands::Remove { alias } => remove_fork(&alias).await,
        ForkCommands::List => list_forks().await,
    }
}

async fn add_fork(alias: &str, git_url: &str) -> Result<()> {
    info!("Adding fork: {} -> {}", alias, git_url);

    // Validate git URL format
    if !git_url.ends_with(".git") {
        anyhow::bail!(
            "Invalid Git URL: {}. URL must end with '.git'",
            git_url
        );
    }

    // Read global config
    let mut config = GlobalConfig::read().await?;

    // Add the fork
    config.add_fork(alias.to_string(), git_url.to_string())
        .context("Failed to add fork")?;

    // Save updated config
    config.save().await
        .context("Failed to save global config")?;

    println!("✓ Fork '{}' added successfully", alias);
    println!("  Repository: {}", git_url);
    println!("\nYou can now use:");
    println!("  fvm-rs install {}/stable", alias);
    println!("  fvm-rs install {}/3.24.0", alias);
    println!("  fvm-rs use {}/stable", alias);

    Ok(())
}

async fn remove_fork(alias: &str) -> Result<()> {
    info!("Removing fork: {}", alias);

    // Read global config
    let mut config = GlobalConfig::read().await?;

    // Remove the fork
    config.remove_fork(alias)
        .context("Failed to remove fork")?;

    // Save updated config
    config.save().await
        .context("Failed to save global config")?;

    println!("✓ Fork '{}' removed successfully", alias);

    Ok(())
}

async fn list_forks() -> Result<()> {
    info!("Listing configured forks");

    // Read global config
    let config = GlobalConfig::read().await?;

    let forks = config.list_forks();

    if forks.is_empty() {
        println!("No forks configured.");
        println!("\nTo add a fork, run:");
        println!("  fvm-rs fork add <alias> <git-url>");
        println!("\nExample:");
        println!("  fvm-rs fork add mycompany https://github.com/mycompany/flutter.git");
        return Ok(());
    }

    println!("Configured forks:\n");

    // Display forks in a table-like format
    for fork in &forks {
        println!("  {} → {}", fork.name, fork.url);
    }

    println!("\nTotal: {} fork(s)", forks.len());

    Ok(())
}
