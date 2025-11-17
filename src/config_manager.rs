use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::debug;

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

/// Write project configuration to both .fvmrc and .fvm/fvm_config.json
///
/// This function writes two config files for FVM compatibility:
/// 1. .fvmrc in the project root (primary format)
/// 2. .fvm/fvm_config.json (legacy format for backward compatibility)
pub async fn write_project_config(project_root: &Path, version: &str) -> Result<()> {
    let config = ProjectConfig::new(version);

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
