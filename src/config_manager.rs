use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

use crate::utils;

/// Main project configuration format (.fvmrc)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    /// Flutter SDK version
    pub flutter: String,

    /// Optional flavors mapping (flavor_name -> version)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavors: Option<HashMap<String, String>>,
}

/// Legacy project configuration format (.fvm/fvm_config.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
struct LegacyProjectConfig {
    #[serde(rename = "flutterSdkVersion")]
    flutter_sdk_version: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    flavors: Option<HashMap<String, String>>,
}

impl ProjectConfig {
    /// Create a new minimal project config with just the Flutter version
    pub fn new(version: impl Into<String>) -> Self {
        Self {
            flutter: version.into(),
            flavors: None,
        }
    }

    /// Convert to legacy format for backward compatibility
    fn to_legacy(&self) -> LegacyProjectConfig {
        LegacyProjectConfig {
            flutter_sdk_version: self.flutter.clone(),
            flavors: self.flavors.clone(),
        }
    }

    /// Convert from legacy format
    fn from_legacy(legacy: LegacyProjectConfig) -> Self {
        Self {
            flutter: legacy.flutter_sdk_version,
            flavors: legacy.flavors,
        }
    }
}

/// Validate that a flavor name is not a channel name
///
/// Channel names (stable, beta, master, dev) cannot be used as flavor names
/// to avoid confusion. Returns an error if the name is a channel.
pub fn validate_flavor_name(flavor_name: &str) -> Result<()> {
    if is_channel(flavor_name) {
        anyhow::bail!(
            "Cannot use channel name '{}' as a flavor name. \
            Flavors must have unique names that are not channels (stable, beta, master, dev).",
            flavor_name
        );
    }
    Ok(())
}

/// Update project configuration with optional main version and flavor updates
///
/// This function intelligently merges updates with existing config:
/// - If `main_version` is provided, updates the main `flutter` field
/// - If `flavor` is provided, adds/updates that specific flavor (merges with existing flavors)
/// - Preserves all existing config that isn't being updated
///
/// Writes to both .fvmrc and .fvm/fvm_config.json for FVM compatibility.
pub async fn update_project_config(
    project_root: &Path,
    main_version: Option<&str>,
    flavor: Option<(&str, &str)>, // (flavor_name, flavor_version)
) -> Result<()> {
    // Read existing config or start with empty
    let mut config = read_project_config(project_root)
        .await?
        .unwrap_or_else(|| ProjectConfig::new(""));

    // Update main version if provided
    if let Some(version) = main_version {
        debug!("Updating main Flutter version to: {}", version);
        config.flutter = version.to_string();
    }

    // Update flavor if provided
    if let Some((flavor_name, flavor_version)) = flavor {
        debug!("Updating flavor '{}' to version: {}", flavor_name, flavor_version);

        // Validate flavor name
        validate_flavor_name(flavor_name)?;

        // Get existing flavors or create new map
        let mut flavors = config.flavors.take().unwrap_or_default();

        // Add/update the flavor
        flavors.insert(flavor_name.to_string(), flavor_version.to_string());

        // Store back (only if not empty)
        config.flavors = if flavors.is_empty() {
            None
        } else {
            Some(flavors)
        };
    }

    // Write both config files
    write_config_files(project_root, &config).await
}

/// Write project configuration to both .fvmrc and .fvm/fvm_config.json
///
/// This function writes two config files for FVM compatibility:
/// 1. .fvmrc in the project root (primary format)
/// 2. .fvm/fvm_config.json (legacy format for backward compatibility)
pub async fn write_project_config(project_root: &Path, version: &str) -> Result<()> {
    let config = ProjectConfig::new(version);
    write_config_files(project_root, &config).await
}

/// Internal helper to write both config files
async fn write_config_files(project_root: &Path, config: &ProjectConfig) -> Result<()> {
    // Write .fvmrc (primary format)
    let fvmrc_path = project_root.join(".fvmrc");
    let fvmrc_json = serde_json::to_string_pretty(&config)
        .context("Failed to serialize .fvmrc config")?;

    debug!("Writing .fvmrc to: {}", fvmrc_path.display());
    fs::write(&fvmrc_path, fvmrc_json)
        .await
        .context("Failed to write .fvmrc")?;

    // Write .fvm/fvm_config.json (legacy format)
    let fvm_dir = project_root.join(".fvm");
    fs::create_dir_all(&fvm_dir)
        .await
        .context("Failed to create .fvm directory")?;

    let legacy_path = fvm_dir.join("fvm_config.json");
    let legacy_config = config.to_legacy();
    let legacy_json = serde_json::to_string_pretty(&legacy_config)
        .context("Failed to serialize legacy config")?;

    debug!("Writing legacy config to: {}", legacy_path.display());
    fs::write(&legacy_path, legacy_json)
        .await
        .context("Failed to write .fvm/fvm_config.json")?;

    Ok(())
}

/// Read project configuration from either .fvmrc or .fvm/fvm_config.json
///
/// Prefers .fvmrc (primary format) and falls back to .fvm/fvm_config.json (legacy).
/// Returns None if no config file is found.
pub async fn read_project_config(project_root: &Path) -> Result<Option<ProjectConfig>> {
    // Try .fvmrc first (primary format)
    let fvmrc_path = project_root.join(".fvmrc");
    if fvmrc_path.exists() {
        debug!("Reading config from: {}", fvmrc_path.display());
        let contents = fs::read_to_string(&fvmrc_path)
            .await
            .context("Failed to read .fvmrc")?;

        let config: ProjectConfig = serde_json::from_str(&contents)
            .context("Failed to parse .fvmrc")?;

        return Ok(Some(config));
    }

    // Fall back to .fvm/fvm_config.json (legacy format)
    let legacy_path = project_root.join(".fvm/fvm_config.json");
    if legacy_path.exists() {
        debug!("Reading legacy config from: {}", legacy_path.display());
        let contents = fs::read_to_string(&legacy_path)
            .await
            .context("Failed to read .fvm/fvm_config.json")?;

        let legacy_config: LegacyProjectConfig = serde_json::from_str(&contents)
            .context("Failed to parse .fvm/fvm_config.json")?;

        return Ok(Some(ProjectConfig::from_legacy(legacy_config)));
    }

    // No config found
    debug!("No FVM config found in: {}", project_root.display());
    Ok(None)
}

/// Get the Flutter version for the current project
///
/// Searches for FVM config starting from the current directory and walking up.
pub async fn get_project_flutter_version() -> Result<Option<String>> {
    let project_root = find_project_root().await?;

    if let Some(root) = project_root {
        let config = read_project_config(&root).await?;
        Ok(config.map(|c| c.flutter))
    } else {
        Ok(None)
    }
}

/// Get the global Flutter version with smart fallback
///
/// Priority:
/// 1. ~/.fvm-rs/default (fvm-rs global version, takes precedence)
/// 2. ~/.fvm/default (original FVM global version, for compatibility)
///
/// Returns the version name if a global version is configured.
pub async fn get_global_flutter_version() -> Result<Option<String>> {
    // Get home directory
    let home = dirs::home_dir()
        .context("Failed to get home directory")?;

    // Check ~/.fvm-rs/default first (takes precedence)
    let fvm_rs_default = home.join(".fvm-rs/default");
    if let Ok(target) = tokio::fs::read_link(&fvm_rs_default).await {
        debug!("Found global version at: {}", fvm_rs_default.display());

        // Extract version name from symlink target
        // Target format: ~/.fvm-rs/flutter/{version}
        if let Some(version) = target.file_name() {
            let version_str = version.to_string_lossy().to_string();
            debug!("Global version (fvm-rs): {}", version_str);
            return Ok(Some(version_str));
        }
    }

    // Fall back to ~/.fvm/default (for compatibility with original FVM)
    let fvm_default = home.join(".fvm/default");
    if let Ok(target) = tokio::fs::read_link(&fvm_default).await {
        debug!("Found global version at: {}", fvm_default.display());

        // Extract version name from symlink target
        // Target format: ~/.fvm/versions/{version}
        if let Some(version) = target.file_name() {
            let version_str = version.to_string_lossy().to_string();
            debug!("Global version (fvm fallback): {}", version_str);
            return Ok(Some(version_str));
        }
    }

    debug!("No global version configured");
    Ok(None)
}

/// Check if a version string is a channel (stable, beta, master) vs a release (3.24.0)
///
/// Returns true for channels, false for release versions.
pub fn is_channel(version: &str) -> bool {
    matches!(version, "stable" | "beta" | "master" | "dev")
}

/// Check if the user is trying to run `flutter upgrade` and protect against it
///
/// This prevents users from accidentally upgrading a pinned release version,
/// which would be meaningless. Only channel versions (stable, beta, master) can be upgraded.
///
/// Returns an error if upgrade is attempted on a non-channel version.
pub async fn check_flutter_upgrade(args: &[String]) -> Result<()> {
    // Only check if first argument is "upgrade"
    if args.is_empty() || args[0] != "upgrade" {
        return Ok(());
    }

    debug!("Detected 'flutter upgrade' command, checking version type");

    // Get the current version (project has priority, then global)
    let version = get_project_flutter_version().await?
        .or_else(|| {
            // Use blocking task for async function in sync context
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(get_global_flutter_version())
                    .ok()
                    .flatten()
            })
        });

    if let Some(version_name) = version {
        debug!("Current version: {}", version_name);

        // Only allow upgrade for channel versions
        if !is_channel(&version_name) {
            anyhow::bail!(
                "You should not upgrade a release version. \
                Please install a channel (stable, beta, master) instead to upgrade it."
            );
        }

        debug!("Version is a channel, upgrade allowed");
    } else {
        debug!("No version configured, allowing system Flutter upgrade");
    }

    Ok(())
}

/// Find the project root by walking up the directory tree looking for FVM config
///
/// Returns the directory containing .fvmrc or .fvm/fvm_config.json, or None if not found.
pub async fn find_project_root() -> Result<Option<PathBuf>> {
    let mut current = std::env::current_dir()
        .context("Failed to get current directory")?;

    loop {
        debug!("Checking for FVM config in: {}", current.display());

        // Check for .fvmrc or .fvm/fvm_config.json
        let fvmrc_path = current.join(".fvmrc");
        let legacy_path = current.join(".fvm/fvm_config.json");

        if fvmrc_path.exists() || legacy_path.exists() {
            debug!("Found FVM config in: {}", current.display());
            return Ok(Some(current));
        }

        // Move up one directory
        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
        } else {
            // Reached root without finding config
            debug!("No FVM config found in directory tree");
            return Ok(None);
        }
    }
}

/// Global configuration for fvm-rs
///
/// Stored in ~/.fvm-rs/.fvmrc on all platforms
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalConfig {
    /// Custom path where fvm-rs will cache Flutter versions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_path: Option<String>,

    /// Enable/disable git cache for faster version installs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_git_cache: Option<bool>,

    /// Path where local Git reference cache is stored
    #[serde(skip_serializing_if = "Option::is_none")]
    pub git_cache_path: Option<String>,

    /// Flutter repository Git URL to clone from
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flutter_url: Option<String>,

    /// Disable automatic update checking for fvm-rs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable_update_check: Option<bool>,
}

impl GlobalConfig {
    /// Read global config from disk
    ///
    /// Returns default config if file doesn't exist.
    pub async fn read() -> Result<Self> {
        let config_path = utils::get_global_config_path()?;

        if !config_path.exists() {
            debug!("No global config found, using defaults");
            return Ok(Self::default());
        }

        debug!("Reading global config from: {}", config_path.display());
        let contents = fs::read_to_string(&config_path)
            .await
            .context("Failed to read global config")?;

        let config: GlobalConfig = serde_json::from_str(&contents)
            .context("Failed to parse global config")?;

        Ok(config)
    }

    /// Save global config to disk
    pub async fn save(&self) -> Result<()> {
        let config_path = utils::get_global_config_path()?;

        // Create parent directory if needed
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                debug!("Creating config directory: {}", parent.display());
                fs::create_dir_all(parent)
                    .await
                    .context("Failed to create config directory")?;
            }
        }

        let json = serde_json::to_string_pretty(self)
            .context("Failed to serialize global config")?;

        debug!("Writing global config to: {}", config_path.display());
        fs::write(&config_path, json)
            .await
            .context("Failed to write global config")?;

        Ok(())
    }

    /// Get cache path with fallback to env var and default
    pub fn get_cache_path(&self) -> Result<PathBuf> {
        // Priority: config file -> FVM_CACHE_PATH env -> FVM_HOME env -> default
        if let Some(path) = &self.cache_path {
            return Ok(PathBuf::from(path));
        }

        if let Ok(path) = std::env::var("FVM_CACHE_PATH") {
            debug!("Using cache path from FVM_CACHE_PATH: {}", path);
            return Ok(PathBuf::from(path));
        }

        if let Ok(path) = std::env::var("FVM_HOME") {
            debug!("Using cache path from FVM_HOME (legacy): {}", path);
            return Ok(PathBuf::from(path));
        }

        // Default: ~/.fvm-rs
        utils::get_fvm_dir()
    }

    /// Get git cache enabled status with fallback to env var and default
    pub fn get_use_git_cache(&self) -> bool {
        // Priority: config file -> FVM_USE_GIT_CACHE env -> default (true)
        if let Some(value) = self.use_git_cache {
            return value;
        }

        if let Ok(value) = std::env::var("FVM_USE_GIT_CACHE") {
            return value.to_lowercase() == "true" || value == "1";
        }

        true // Default: enabled
    }

    /// Get git cache path with fallback to env var and default
    pub fn get_git_cache_path(&self) -> Result<PathBuf> {
        // Priority: config file -> FVM_GIT_CACHE_PATH env -> default (cache_path/shared/flutter)
        if let Some(path) = &self.git_cache_path {
            return Ok(PathBuf::from(path));
        }

        if let Ok(path) = std::env::var("FVM_GIT_CACHE_PATH") {
            debug!("Using git cache path from FVM_GIT_CACHE_PATH: {}", path);
            return Ok(PathBuf::from(path));
        }

        // Default: {cache_path}/shared/flutter
        let cache_path = self.get_cache_path()?;
        Ok(cache_path.join("shared/flutter"))
    }

    /// Get Flutter repository URL with fallback to env var and default
    pub fn get_flutter_url(&self) -> String {
        // Priority: config file -> FVM_FLUTTER_URL/FLUTTER_GIT_URL env -> default
        if let Some(url) = &self.flutter_url {
            return url.clone();
        }

        if let Ok(url) = std::env::var("FVM_FLUTTER_URL") {
            debug!("Using Flutter URL from FVM_FLUTTER_URL: {}", url);
            return url;
        }

        if let Ok(url) = std::env::var("FLUTTER_GIT_URL") {
            debug!("Using Flutter URL from FLUTTER_GIT_URL: {}", url);
            return url;
        }

        "https://github.com/flutter/flutter.git".to_string()
    }

    /// Get update check enabled status
    pub fn get_update_check_enabled(&self) -> bool {
        // If disable_update_check is Some(true), return false (disabled)
        // Otherwise return true (enabled by default)
        !self.disable_update_check.unwrap_or(false)
    }

    /// Check if config is empty (all fields are None)
    pub fn is_empty(&self) -> bool {
        self.cache_path.is_none()
            && self.use_git_cache.is_none()
            && self.git_cache_path.is_none()
            && self.flutter_url.is_none()
            && self.disable_update_check.is_none()
    }
}
