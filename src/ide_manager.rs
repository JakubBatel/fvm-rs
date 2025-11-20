use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::Path;
use tokio::fs;
use tracing::debug;

/// Update VS Code settings.json to use the Flutter SDK from .fvm
///
/// Updates .vscode/settings.json with the dart.flutterSdkPath setting.
/// Uses relative path ".fvm/flutter_sdk" for portability.
pub async fn update_vscode_settings(project_root: &Path) -> Result<()> {
    let vscode_dir = project_root.join(".vscode");
    let settings_path = vscode_dir.join("settings.json");

    debug!("Updating VS Code settings at: {}", settings_path.display());

    // Ensure .vscode directory exists
    fs::create_dir_all(&vscode_dir)
        .await
        .context("Failed to create .vscode directory")?;

    // Read existing settings or start with empty object
    let mut settings: Value = if settings_path.exists() {
        let contents = fs::read_to_string(&settings_path)
            .await
            .context("Failed to read .vscode/settings.json")?;

        debug!("Found existing VS Code settings, merging");
        serde_json::from_str(&contents)
            .context("Failed to parse .vscode/settings.json")?
    } else {
        debug!("Creating new VS Code settings");
        json!({})
    };

    // Update dart.flutterSdkPath
    if let Some(obj) = settings.as_object_mut() {
        obj.insert(
            "dart.flutterSdkPath".to_string(),
            json!(".fvm/flutter_sdk"),
        );
        debug!("Set dart.flutterSdkPath to .fvm/flutter_sdk");
    }

    // Write back the settings file
    let json_str = serde_json::to_string_pretty(&settings)
        .context("Failed to serialize VS Code settings")?;

    fs::write(&settings_path, json_str)
        .await
        .context("Failed to write .vscode/settings.json")?;

    Ok(())
}

/// Update VS Code workspace files (.code-workspace) to use the Flutter SDK from .fvm
///
/// Searches for .code-workspace files in the project root and updates them
/// with the dart.flutterSdkPath setting.
pub async fn update_vscode_workspace(project_root: &Path) -> Result<()> {
    // Find all .code-workspace files in project root
    let mut entries = fs::read_dir(project_root)
        .await
        .context("Failed to read project directory")?;

    let mut workspace_files = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("code-workspace") {
            workspace_files.push(path);
        }
    }

    if workspace_files.is_empty() {
        debug!("No .code-workspace files found");
        return Ok(());
    }

    debug!("Found {} .code-workspace file(s)", workspace_files.len());

    // Update each workspace file
    for workspace_path in workspace_files {
        debug!("Updating workspace file: {}", workspace_path.display());

        let contents = fs::read_to_string(&workspace_path)
            .await
            .context("Failed to read .code-workspace file")?;

        let mut workspace: Value = serde_json::from_str(&contents)
            .context("Failed to parse .code-workspace file")?;

        // Update settings.dart.flutterSdkPath
        if let Some(obj) = workspace.as_object_mut() {
            let settings = obj
                .entry("settings")
                .or_insert_with(|| json!({}));

            if let Some(settings_obj) = settings.as_object_mut() {
                settings_obj.insert(
                    "dart.flutterSdkPath".to_string(),
                    json!(".fvm/flutter_sdk"),
                );
                debug!("Updated dart.flutterSdkPath in workspace file");
            }
        }

        // Write back the workspace file
        let json_str = serde_json::to_string_pretty(&workspace)
            .context("Failed to serialize workspace file")?;

        fs::write(&workspace_path, json_str)
            .await
            .context("Failed to write .code-workspace file")?;
    }

    Ok(())
}

/// Update IntelliJ/Android Studio settings to use the Flutter SDK from .fvm
///
/// Updates two files:
/// 1. android/local.properties - Adds flutter.sdk path
/// 2. .idea/libraries/Dart_SDK.xml - Updates Dart SDK library path
pub async fn update_intellij_settings(project_root: &Path) -> Result<()> {
    // Update android/local.properties
    update_local_properties(project_root).await?;

    // Update .idea/libraries/Dart_SDK.xml
    update_dart_sdk_xml(project_root).await?;

    Ok(())
}

/// Update android/local.properties with Flutter SDK path
async fn update_local_properties(project_root: &Path) -> Result<()> {
    let android_dir = project_root.join("android");

    // Check if android directory exists (not all Flutter projects have it)
    if !android_dir.exists() {
        debug!("No android directory found, skipping local.properties update");
        return Ok(());
    }

    let properties_path = android_dir.join("local.properties");
    debug!("Updating local.properties at: {}", properties_path.display());

    // Read existing properties or start fresh
    let mut lines: Vec<String> = if properties_path.exists() {
        let contents = fs::read_to_string(&properties_path)
            .await
            .context("Failed to read local.properties")?;

        debug!("Found existing local.properties");
        contents.lines().map(|s| s.to_string()).collect()
    } else {
        debug!("Creating new local.properties");
        Vec::new()
    };

    // Remove any existing flutter.sdk line
    lines.retain(|line| !line.trim().starts_with("flutter.sdk"));

    // Add the new flutter.sdk path (absolute path)
    let flutter_sdk_path = project_root.join(".fvm/flutter_sdk");
    let flutter_sdk_str = flutter_sdk_path
        .to_str()
        .context("Invalid Flutter SDK path")?;

    lines.push(format!("flutter.sdk={}", flutter_sdk_str));
    debug!("Set flutter.sdk to {}", flutter_sdk_str);

    // Write back the properties file
    let contents = lines.join("\n") + "\n";
    fs::write(&properties_path, contents)
        .await
        .context("Failed to write local.properties")?;

    Ok(())
}

/// Update .idea/libraries/Dart_SDK.xml with Dart SDK path
async fn update_dart_sdk_xml(project_root: &Path) -> Result<()> {
    let idea_dir = project_root.join(".idea");

    // Check if .idea directory exists (not present in all projects)
    if !idea_dir.exists() {
        debug!("No .idea directory found, skipping Dart_SDK.xml update");
        return Ok(());
    }

    let libraries_dir = idea_dir.join("libraries");
    fs::create_dir_all(&libraries_dir)
        .await
        .context("Failed to create .idea/libraries directory")?;

    let dart_sdk_path = libraries_dir.join("Dart_SDK.xml");
    debug!("Updating Dart_SDK.xml at: {}", dart_sdk_path.display());

    // Build the absolute path to the Dart SDK
    let flutter_sdk_path = project_root.join(".fvm/flutter_sdk");
    let dart_sdk_full_path = flutter_sdk_path.join("bin/cache/dart-sdk");
    let dart_sdk_str = dart_sdk_full_path
        .to_str()
        .context("Invalid Dart SDK path")?;

    // Create the Dart_SDK.xml content
    // This follows IntelliJ's library XML format
    let xml_content = format!(
        r#"<component name="libraryTable">
  <library name="Dart SDK">
    <CLASSES>
      <root url="file://{}/lib/core" />
    </CLASSES>
    <JAVADOC />
    <SOURCES />
  </library>
</component>
"#,
        dart_sdk_str
    );

    debug!("Set Dart SDK path to {}", dart_sdk_str);

    fs::write(&dart_sdk_path, xml_content)
        .await
        .context("Failed to write Dart_SDK.xml")?;

    Ok(())
}
