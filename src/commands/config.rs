use anyhow::Result;
use clap::Args;
use tracing::info;

use crate::config_manager::GlobalConfig;
use crate::utils;

#[derive(Args, Debug, Clone)]
pub struct ConfigArgs {
    /// Set custom cache path for Flutter versions
    #[arg(long)]
    cache_path: Option<String>,

    /// Enable or disable git cache for faster installs
    #[arg(long, value_name = "BOOL")]
    use_git_cache: Option<bool>,

    /// Set custom git cache path
    #[arg(long)]
    git_cache_path: Option<String>,

    /// Set custom Flutter repository URL
    #[arg(long)]
    flutter_url: Option<String>,

    /// Enable or disable automatic update checking
    #[arg(long, value_name = "BOOL")]
    update_check: Option<bool>,
}

impl ConfigArgs {
    /// Check if any config option was explicitly set
    fn has_any_set(&self) -> bool {
        self.cache_path.is_some()
            || self.use_git_cache.is_some()
            || self.git_cache_path.is_some()
            || self.flutter_url.is_some()
            || self.update_check.is_some()
    }
}

pub async fn run(args: ConfigArgs) -> Result<()> {
    if args.has_any_set() {
        // Set mode: update configuration
        set_config(args).await
    } else {
        // Display mode: show current configuration
        display_config().await
    }
}

async fn display_config() -> Result<()> {
    info!("Reading global configuration");

    let config = GlobalConfig::read().await?;
    let config_path = utils::get_global_config_path()?;

    println!("FVM-RS Configuration");
    println!("Located at: {}\n", config_path.display());

    if config.is_empty() {
        println!("No settings have been configured.");
        println!("\nUsing defaults:");
    } else {
        println!("Current settings:");
    }

    // Show effective values (with fallbacks)
    println!("  cachePath: {}", config.get_cache_path()?.display());
    println!("  useGitCache: {}", config.get_use_git_cache());
    println!("  gitCachePath: {}", config.get_git_cache_path()?.display());
    println!("  flutterUrl: {}", config.get_flutter_url());
    println!("  updateCheck: {}", config.get_update_check_enabled());

    if !config.is_empty() {
        println!("\nNote: Values shown include defaults for unset options.");
        println!("Environment variables (FVM_*) can override config file settings.");
    }

    Ok(())
}

async fn set_config(args: ConfigArgs) -> Result<()> {
    info!("Updating global configuration");

    // Read existing config
    let mut config = GlobalConfig::read().await?;

    // Track what's being changed
    let mut changes = Vec::new();

    // Update only the fields that were explicitly set
    if let Some(path) = args.cache_path {
        println!("Setting cache-path to: {}", path);
        config.cache_path = Some(path.clone());
        changes.push(format!("cachePath: {}", path));
    }

    if let Some(enabled) = args.use_git_cache {
        println!("Setting use-git-cache to: {}", enabled);
        config.use_git_cache = Some(enabled);
        changes.push(format!("useGitCache: {}", enabled));
    }

    if let Some(path) = args.git_cache_path {
        println!("Setting git-cache-path to: {}", path);
        config.git_cache_path = Some(path.clone());
        changes.push(format!("gitCachePath: {}", path));
    }

    if let Some(url) = args.flutter_url {
        println!("Setting flutter-url to: {}", url);
        config.flutter_url = Some(url.clone());
        changes.push(format!("flutterUrl: {}", url));
    }

    if let Some(enabled) = args.update_check {
        println!("Setting update-check to: {}", enabled);
        config.disable_update_check = Some(!enabled); // Note: inverted logic
        changes.push(format!("updateCheck: {}", enabled));
    }

    // Save configuration
    println!("\nSaving settings...");
    config.save().await?;

    println!("✓ Settings saved successfully!");

    if !changes.is_empty() {
        println!("\nUpdated:");
        for change in changes {
            println!("  • {}", change);
        }
    }

    Ok(())
}
