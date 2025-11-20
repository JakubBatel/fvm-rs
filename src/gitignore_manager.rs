use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;
use tracing::debug;

/// Update .fvm/.gitignore to ignore the flutter_sdk symlink
///
/// This ensures the Flutter SDK symlink is not committed to version control.
/// The .fvm directory itself should typically be gitignored, but this provides
/// an extra layer of protection.
pub async fn update_fvm_gitignore(project_root: &Path) -> Result<()> {
    let fvm_dir = project_root.join(".fvm");
    let gitignore_path = fvm_dir.join(".gitignore");

    debug!("Updating .fvm/.gitignore at: {}", gitignore_path.display());

    // Ensure .fvm directory exists
    fs::create_dir_all(&fvm_dir)
        .await
        .context("Failed to create .fvm directory")?;

    // Read existing .gitignore if it exists
    let mut entries = if gitignore_path.exists() {
        let contents = fs::read_to_string(&gitignore_path)
            .await
            .context("Failed to read .fvm/.gitignore")?;

        debug!("Found existing .fvm/.gitignore, preserving entries");
        contents.lines().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        debug!("Creating new .fvm/.gitignore");
        Vec::new()
    };

    // Add flutter_sdk entry if not already present
    const FLUTTER_SDK_ENTRY: &str = "flutter_sdk";
    if !entries.iter().any(|line| line.trim() == FLUTTER_SDK_ENTRY) {
        debug!("Adding 'flutter_sdk' entry to .fvm/.gitignore");
        entries.push(FLUTTER_SDK_ENTRY.to_string());
    } else {
        debug!("'flutter_sdk' entry already exists in .fvm/.gitignore");
    }

    // Write back the .gitignore file
    let contents = entries.join("\n") + "\n";
    fs::write(&gitignore_path, contents)
        .await
        .context("Failed to write .fvm/.gitignore")?;

    Ok(())
}

/// Update project root .gitignore to include .fvm directory
///
/// This is optional and should only be called when the user has enabled
/// the `updateGitIgnore` config option.
pub async fn update_project_gitignore(project_root: &Path) -> Result<()> {
    let gitignore_path = project_root.join(".gitignore");

    debug!("Updating project .gitignore at: {}", gitignore_path.display());

    // Read existing .gitignore if it exists
    let mut entries = if gitignore_path.exists() {
        let contents = fs::read_to_string(&gitignore_path)
            .await
            .context("Failed to read .gitignore")?;

        debug!("Found existing .gitignore, preserving entries");
        contents.lines().map(|s| s.to_string()).collect::<Vec<_>>()
    } else {
        debug!("Creating new .gitignore");
        Vec::new()
    };

    // Add .fvm/ entry if not already present
    const FVM_ENTRY: &str = ".fvm/";
    if !entries.iter().any(|line| line.trim() == FVM_ENTRY || line.trim() == ".fvm") {
        debug!("Adding '.fvm/' entry to project .gitignore");
        entries.push(FVM_ENTRY.to_string());
    } else {
        debug!("'.fvm/' entry already exists in project .gitignore");
    }

    // Write back the .gitignore file
    let contents = entries.join("\n") + "\n";
    fs::write(&gitignore_path, contents)
        .await
        .context("Failed to write .gitignore")?;

    Ok(())
}
