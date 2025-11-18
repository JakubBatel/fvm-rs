use anyhow::{Context, Result};
use clap::Args;
use std::env;
use tracing::info;

use crate::{config_manager, utils};

#[derive(Debug, Clone, Args)]
pub struct DoctorArgs {}

pub async fn run(_args: DoctorArgs) -> Result<()> {
    info!("Running FVM doctor diagnostics");

    println!("FVM Doctor");
    println!("══════════════════════════════════════════════════");
    println!();

    // Project Info Section
    print_project_info().await?;
    println!();

    // IDE Integration Section
    print_ide_integration().await?;
    println!();

    // Environment Section
    print_environment_info().await?;
    println!();

    println!("══════════════════════════════════════════════════");
    info!("Doctor diagnostics completed");

    Ok(())
}

async fn print_project_info() -> Result<()> {
    println!("📋 Project Information");
    println!("──────────────────────────────────────────────────");

    // Current directory
    let current_dir = env::current_dir().context("Failed to get current directory")?;
    println!("  Directory:          {}", current_dir.display());

    // Check if FVM config exists
    let config = config_manager::read_project_config(&current_dir).await?;
    if let Some(cfg) = config {
        println!("  FVM Configured:     ✓ Yes");
        println!("  Flutter Version:    {}", cfg.flutter);

        if let Some(flavors) = &cfg.flavors {
            println!("  Flavors:            {} configured", flavors.len());
            for (name, version) in flavors {
                println!("    • {}: {}", name, version);
            }
        } else {
            println!("  Flavors:            None");
        }

        // Check if .fvmrc exists
        let fvmrc_path = current_dir.join(".fvmrc");
        if fvmrc_path.exists() {
            println!("  Config File:        .fvmrc");
        } else {
            println!("  Config File:        .fvm/fvm_config.json (legacy)");
        }

        // Check if version is installed
        let version_dir = utils::flutter_version_dir(&cfg.flutter)?;
        if version_dir.exists() {
            println!("  Version Installed:  ✓ Yes");
        } else {
            println!("  Version Installed:  ✗ No (run: fvm-rs install {})", cfg.flutter);
        }
    } else {
        println!("  FVM Configured:     ✗ No");
        println!("  Hint:               Run 'fvm-rs use <version>' to configure this project");
    }

    // Check if this is a Flutter project
    let pubspec_path = current_dir.join("pubspec.yaml");
    if pubspec_path.exists() {
        println!("  Flutter Project:    ✓ Yes");
    } else {
        println!("  Flutter Project:    ⚠ No pubspec.yaml found");
    }

    Ok(())
}

async fn print_ide_integration() -> Result<()> {
    println!("🔧 IDE Integration");
    println!("──────────────────────────────────────────────────");

    let current_dir = env::current_dir().context("Failed to get current directory")?;

    // VS Code settings
    let vscode_settings = current_dir.join(".vscode/settings.json");
    if vscode_settings.exists() {
        println!("  VS Code Settings:   ✓ Found");
        // TODO: Validate that dart.flutterSdkPath is correct
    } else {
        println!("  VS Code Settings:   ✗ Not found");
        println!("    Hint:             Create .vscode/settings.json with:");
        println!("                      {{\"dart.flutterSdkPath\": \".fvm/flutter_sdk\"}}");
    }

    // IntelliJ/Android Studio settings
    let idea_dir = current_dir.join(".idea");
    if idea_dir.exists() {
        println!("  IntelliJ IDEA:      ✓ .idea directory found");
        // TODO: Validate libraries/Dart_SDK.xml
    } else {
        println!("  IntelliJ IDEA:      ✗ .idea directory not found");
    }

    // Check .gitignore
    let gitignore = current_dir.join(".fvm/.gitignore");
    if gitignore.exists() {
        println!("  .fvm/.gitignore:    ✓ Present");
    } else {
        println!("  .fvm/.gitignore:    ⚠ Missing");
    }

    // Check .fvm/flutter_sdk symlink (legacy format)
    let flutter_sdk_link = current_dir.join(".fvm/flutter_sdk");
    if flutter_sdk_link.exists() {
        if flutter_sdk_link.is_symlink() {
            let target = tokio::fs::read_link(&flutter_sdk_link).await?;
            println!("  Flutter SDK Link:   ✓ Valid symlink");
            println!("    Target:           {}", target.display());
        } else {
            println!("  Flutter SDK Link:   ⚠ Exists but not a symlink");
        }
    } else {
        println!("  Flutter SDK Link:   ✗ Not found (.fvm/flutter_sdk)");
        println!("    Note:             fvm-rs uses direct config, symlink not required");
    }

    Ok(())
}

async fn print_environment_info() -> Result<()> {
    println!("🌍 Environment");
    println!("──────────────────────────────────────────────────");

    // Platform info
    println!("  Platform:           {} ({})", env::consts::OS, env::consts::ARCH);

    // FVM cache directory
    let fvm_dir = utils::get_fvm_dir()?;
    println!("  FVM Cache:          {}", fvm_dir.display());
    if fvm_dir.exists() {
        println!("  Cache Exists:       ✓ Yes");
    } else {
        println!("  Cache Exists:       ✗ No");
    }

    // Global version
    let global_version = config_manager::get_global_flutter_version().await?;
    if let Some(version) = global_version {
        println!("  Global Version:     {}", version);
    } else {
        println!("  Global Version:     Not set");
    }

    // Flutter in PATH
    match which::which("flutter") {
        Ok(flutter_path) => {
            println!("  Flutter in PATH:    ✓ {}", flutter_path.display());
        }
        Err(_) => {
            println!("  Flutter in PATH:    ✗ Not found");
        }
    }

    // Environment variables
    println!("  Environment Variables:");
    print_env_var("FVM_CACHE_PATH");
    print_env_var("FVM_USE_GIT_CACHE");
    print_env_var("FVM_GIT_CACHE_PATH");
    print_env_var("FVM_FLUTTER_URL");
    print_env_var("FVM_HOME");

    Ok(())
}

fn print_env_var(name: &str) {
    if let Ok(value) = env::var(name) {
        println!("    {:<20} {}", name, value);
    } else {
        println!("    {:<20} (not set)", name);
    }
}
