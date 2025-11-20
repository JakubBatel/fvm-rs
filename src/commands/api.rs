use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use serde::Serialize;
use serde_json::json;
use std::env;
use std::path::PathBuf;
use tracing::info;

use crate::{config_manager, sdk_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct ApiArgs {
    /// Output compact JSON (no pretty formatting)
    #[arg(long, short = 'c', global = true)]
    compress: bool,

    #[command(subcommand)]
    pub command: ApiCommands,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ApiCommands {
    /// Returns installed Flutter SDK versions as JSON
    List {
        /// Skip calculating directory sizes (faster)
        #[arg(long, short = 's')]
        skip_size_calculation: bool,
    },
    /// Returns available Flutter SDK releases as JSON
    Releases {
        /// Limit number of releases returned
        #[arg(long)]
        limit: Option<usize>,

        /// Filter by channel (stable, beta, dev)
        #[arg(long)]
        filter_channel: Option<String>,
    },
    /// Returns environment information as JSON
    Context,
    /// Returns project configuration as JSON
    Project {
        /// Project path (defaults to current directory)
        #[arg(long, short = 'p')]
        path: Option<PathBuf>,
    },
}

pub async fn run(args: ApiArgs) -> Result<()> {
    let result = match args.command {
        ApiCommands::List {
            skip_size_calculation,
        } => api_list(skip_size_calculation).await?,
        ApiCommands::Releases {
            limit,
            filter_channel,
        } => api_releases(limit, filter_channel.as_deref()).await?,
        ApiCommands::Context => api_context().await?,
        ApiCommands::Project { path } => api_project(path).await?,
    };

    // Output JSON
    let json_str = if args.compress {
        serde_json::to_string(&result)?
    } else {
        serde_json::to_string_pretty(&result)?
    };

    println!("{}", json_str);

    Ok(())
}

#[derive(Debug, Serialize)]
struct VersionInfo {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<String>,
}

async fn api_list(skip_size: bool) -> Result<serde_json::Value> {
    info!("API: Listing installed versions");

    let versions = sdk_manager::list_installed_versions().await?;
    let mut version_infos = Vec::new();

    for version in versions {
        let size = if skip_size {
            None
        } else {
            // Calculate directory size (simplified - would need proper implementation)
            None // TODO: Implement size calculation if needed
        };

        version_infos.push(VersionInfo {
            name: version,
            size,
        });
    }

    Ok(json!({
        "versions": version_infos,
        "total": version_infos.len(),
    }))
}

async fn api_releases(limit: Option<usize>, filter_channel: Option<&str>) -> Result<serde_json::Value> {
    info!("API: Fetching available releases");

    let releases = sdk_manager::list_available_versions().await?;

    let mut filtered_releases: Vec<_> = releases.releases.iter().collect();

    // Filter by channel if specified
    if let Some(channel) = filter_channel {
        filtered_releases.retain(|r| r.channel == channel);
    }

    // Apply limit if specified
    if let Some(max) = limit {
        filtered_releases.truncate(max);
    }

    Ok(json!({
        "current": {
            "stable": releases.current_releases.stable.version,
            "beta": releases.current_releases.beta.version,
            "dev": releases.current_releases.dev.version,
        },
        "releases": filtered_releases,
        "total": filtered_releases.len(),
    }))
}

async fn api_context() -> Result<serde_json::Value> {
    info!("API: Fetching environment context");

    let config = config_manager::GlobalConfig::read().await?;
    let fvm_dir = utils::get_fvm_dir()?;
    let global_version = config_manager::get_global_flutter_version().await?;

    // Check for project version
    let project_version = config_manager::get_project_flutter_version().await?;
    let project_root = config_manager::find_project_root().await?;

    Ok(json!({
        "fvmCachePath": fvm_dir.to_string_lossy(),
        "globalFlutterVersion": global_version,
        "projectFlutterVersion": project_version,
        "projectRoot": project_root.map(|p| p.to_string_lossy().to_string()),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "config": {
            "cachePath": config.cache_path,
            "useGitCache": config.use_git_cache,
            "gitCachePath": config.git_cache_path,
            "flutterUrl": config.flutter_url,
            "updateVscodeSettings": config.update_vscode_settings,
            "updateGitignore": config.update_gitignore,
            "forks": config.forks,
        },
    }))
}

async fn api_project(path: Option<PathBuf>) -> Result<serde_json::Value> {
    info!("API: Fetching project configuration");

    let project_root = if let Some(p) = path {
        p
    } else {
        env::current_dir().context("Failed to get current directory")?
    };

    let project_config = config_manager::read_project_config(&project_root)
        .await?
        .context("No FVM project configuration found")?;

    Ok(json!({
        "projectRoot": project_root.to_string_lossy(),
        "flutterVersion": project_config.flutter,
        "flavors": project_config.flavors,
    }))
}
