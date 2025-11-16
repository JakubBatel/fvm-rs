use crate::utils;
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use git2::{FetchOptions, Repository, build::RepoBuilder};
use serde::Deserialize;
use std::{collections::HashSet, io::Cursor, path::PathBuf};
use tokio::{fs, task};
use zip::ZipArchive;


#[derive(Clone, Debug, Deserialize)]
pub struct FlutterRelease {
    pub hash: String,
    pub channel: String,
    pub version: String,
    pub dart_sdk_version: Option<String>,
    pub release_date: DateTime<Utc>,
}

pub struct CurrentReleases {
    pub stable: FlutterRelease,
    pub beta: FlutterRelease,
    pub dev: FlutterRelease,
}

pub struct FlutterReleases {
    pub current_releases: CurrentReleases,
    pub releases: Vec<FlutterRelease>,
}

#[derive(Debug, Deserialize)]
struct CurrentReleasesResponse {
    stable: String,
    beta: String,
    dev: String,
}

#[derive(Debug, Deserialize)]
struct FlutterReleasesResponse {
    current_release: CurrentReleasesResponse,
    releases: Vec<FlutterRelease>,
}

pub async fn ensure_installed(version: &str) -> Result<()> {
    if !verify_installed(version)? {
        install(version).await?;
    }
    return Ok(());
}

pub async fn list_installed_versions() -> Result<Vec<String>> {
    let flutter_root = utils::flutter_dir()?;

    if !flutter_root.exists() {
        return Ok(vec![]);
    }

    let mut entries = fs::read_dir(flutter_root).await?;
    let mut versions = vec![];

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if fs::metadata(&path).await?.is_dir() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                versions.push(name.to_string());
            }
        }
    }

    return Ok(versions);
}

pub async fn list_available_versions() -> Result<FlutterReleases> {
    let platform = std::env::consts::OS;

    let url = format!(
        "https://storage.googleapis.com/flutter_infra_release/releases/releases_{platform}.json"
    );
    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch list of available versions")?
        .error_for_status()?;

    let parsed: FlutterReleasesResponse = response.json().await.context("Invalid JSON")?;

    let mut seen = HashSet::new();
    let mut versions = vec![];

    for release in parsed.releases {
        if seen.insert(release.hash.clone()) {
            versions.push(release);
        }
    }

    return Ok(FlutterReleases {
        current_releases: CurrentReleases {
            stable: versions
                .iter()
                .find(|r| r.hash == parsed.current_release.stable)
                .unwrap()
                .clone(),
            beta: versions
                .iter()
                .find(|r| r.hash == parsed.current_release.beta)
                .unwrap()
                .clone(),
            dev: versions
                .iter()
                .find(|r| r.hash == parsed.current_release.dev)
                .unwrap()
                .clone(),
        },
        releases: versions,
    });
}

/// Get the engine hash used by a specific Flutter version
/// Returns None if the version is not installed or the engine.stamp file is missing
pub async fn get_engine_hash_for_version(version: &str) -> Result<Option<String>> {
    let flutter_dir = utils::flutter_version_dir(version)?;
    let stamp_file = flutter_dir.join("bin").join("cache").join("engine.stamp");

    if !stamp_file.exists() {
        return Ok(None);
    }

    match fs::read_to_string(&stamp_file).await {
        Ok(hash) => Ok(Some(hash.trim().to_string())),
        Err(_) => Ok(None),
    }
}

/// Result of cleaning up unused engines
pub struct EngineCleanupResult {
    pub removed_engines: Vec<String>,
    pub failed_removals: Vec<(String, String)>, // (hash, error_message)
}

/// Clean up engine caches that are no longer used by any installed Flutter version
/// Returns details about removed and failed engines
pub async fn cleanup_unused_engines() -> Result<EngineCleanupResult> {
    let engine_dir = utils::shared_engine_dir()?;

    // If the engine directory doesn't exist, nothing to clean up
    if !engine_dir.exists() {
        return Ok(EngineCleanupResult {
            removed_engines: vec![],
            failed_removals: vec![],
        });
    }

    // Collect all engine hashes currently in use by installed Flutter versions
    let installed_versions = list_installed_versions().await?;
    let mut used_engines = HashSet::new();

    for version in installed_versions {
        if let Some(hash) = get_engine_hash_for_version(&version).await? {
            used_engines.insert(hash);
        }
    }

    // Find and delete unused engines
    let mut removed_engines = vec![];
    let mut failed_removals = vec![];
    let mut entries = fs::read_dir(&engine_dir).await?;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if let Some(hash) = path.file_name().and_then(|s| s.to_str()) {
            if !used_engines.contains(hash) {
                // This engine is not used by any Flutter version, delete it
                match fs::remove_dir_all(&path).await {
                    Ok(_) => {
                        removed_engines.push(hash.to_string());
                    }
                    Err(e) => {
                        failed_removals.push((hash.to_string(), e.to_string()));
                    }
                }
            }
        }
    }

    return Ok(EngineCleanupResult {
        removed_engines,
        failed_removals,
    });
}

pub async fn uninstall(version: &str) -> Result<Option<String>> {
    let flutter_dir = utils::flutter_version_dir(version)?;

    if !flutter_dir.exists() {
        return Ok(None);
    }

    // Get the engine hash before deleting the directory
    let engine_hash = get_engine_hash_for_version(version).await?;

    // Delete the Flutter directory
    fs::remove_dir_all(&flutter_dir).await?;

    // Remove the worktree from git
    let shared_repo_path = utils::shared_flutter_dir()?;
    let worktree_name = format!("fvm-{}", version);

    // Spawn blocking task for git operations
    let shared_repo_path_clone = shared_repo_path.clone();
    let worktree_name_clone = worktree_name.clone();

    task::spawn_blocking(move || {
        // Open the shared bare repository
        if let Ok(repo) = Repository::open_bare(&shared_repo_path_clone) {
            // Find and remove the worktree
            if let Ok(worktree) = repo.find_worktree(&worktree_name_clone) {
                // The worktree directory is already deleted, but we need to prune it from git's tracking
                // This is safe - if the worktree is already gone, this is a no-op
                let _ = worktree.prune(None);
            }
        }
        Ok::<_, anyhow::Error>(())
    })
    .await??;

    return Ok(engine_hash);
}

fn verify_installed(version: &str) -> Result<bool> {
    let flutter_root = utils::flutter_version_dir(version)?;

    if !flutter_root.exists() {
        return Ok(false);
    }

    let flutter_bin = flutter_root.join("bin").join(if cfg!(windows) {
        "flutter.bat"
    } else {
        "flutter"
    });

    if !flutter_bin.exists() {
        return Ok(false);
    }

    return Ok(true);
}

async fn install(version: &str) -> Result<()> {
    let engine_hash = fetch_engine_hash(version).await?;

    let engine_dir = utils::shared_engine_hash_dir(&engine_hash)?;
    let flutter_dir = utils::flutter_version_dir(version)?;

    let (engine_result, flutter_result) =
        tokio::join!(install_engine(&engine_dir), install_flutter(&flutter_dir),);

    engine_result?;
    flutter_result?;

    link_engine_to_flutter(&engine_dir, &flutter_dir).await?;

    return Ok(());
}

async fn fetch_engine_hash(version: &str) -> Result<String> {
    let url = format!(
        "https://raw.githubusercontent.com/flutter/flutter/{}/bin/internal/engine.version",
        version
    );

    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch engine hash")?
        .error_for_status()?;

    return Ok(response
        .text()
        .await
        .context("Could not read engine.version")?
        .trim()
        .to_string());
}

async fn install_engine(engine_dir: &PathBuf) -> Result<()> {
    if engine_dir.exists() {
        return Ok(());
    }

    let platform = match std::env::consts::OS {
        "macos" => "darwin", // match Flutter conventions
        other => other,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => Ok("x64"),
        "aarch64" | "arm64" | "armv8" => Ok("arm64"),
        other => Err(anyhow!("Unsupported platform {}", other)),
    }?;

    let engine_hash = engine_dir.file_name().unwrap().to_str().unwrap();

    let url = format!(
        "https://storage.googleapis.com/flutter_infra_release/flutter/{}/dart-sdk-{}-{}.zip",
        engine_hash, platform, arch
    );

    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch engine zip")?
        .error_for_status()
        .context("Failed to fetch engine zip")?;

    let bytes = response
        .bytes()
        .await
        .context("Failed to read engine zip")?;

    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    fs::create_dir_all(engine_dir)
        .await
        .context("Failed to create engine dir")?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let path = file.name();

        if let Some(stripped_path) = path.strip_prefix("dart-sdk/") {
            if stripped_path.is_empty() {
                continue;
            }

            let outpath = engine_dir.join(stripped_path);

            if file.is_dir() {
                fs::create_dir_all(&outpath).await?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(&p).await?;
                    }
                }
                let mut outfile = std::fs::File::create(&outpath)?;
                std::io::copy(&mut file, &mut outfile)?;
            }

            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode))?;
                }
            }
        }
    }

    return Ok(());
}

async fn install_flutter(version_dir: &PathBuf) -> Result<()> {
    let url = "https://github.com/flutter/flutter.git";
    let shared_dir = utils::shared_flutter_dir()?;

    let repo = ensure_shared_repo(url, &shared_dir).await?;

    let parent_dir = version_dir.parent().unwrap();
    fs::create_dir_all(parent_dir).await?;

    let version_dir = version_dir.clone();

    task::spawn_blocking(move || {
        let version = version_dir.file_name().unwrap().to_str().unwrap();

        let worktree = repo
            .worktree(&format!("fvm-{}", version), &version_dir, None)
            .context("Failed to create worktree")?;

        let worktree_repo =
            Repository::open(worktree.path()).context("Failed to open worktree repository")?;

        let commit_ref = format!("refs/tags/{}", version);

        let commit = worktree_repo
            .find_reference(&commit_ref)?
            .peel_to_commit()?;

        worktree_repo.checkout_tree(commit.as_object(), None)?;
        worktree_repo.set_head_detached(commit.id())?;

        return Ok::<_, anyhow::Error>(());
    })
    .await??;

    return Ok(());
}

async fn ensure_shared_repo(url: &str, path: &PathBuf) -> Result<git2::Repository> {
    if path.exists() {
        let repo_result = Repository::open_bare(path.clone());
        if let Ok(repo) = repo_result {
            {
                let mut remote = repo.find_remote("origin").context("Failed to get remote")?;

                let mut fetch_options = FetchOptions::new();
                fetch_options.download_tags(git2::AutotagOption::All);

                remote
                    .fetch(
                        &["refs/heads/*:refs/heads/*", "refs/tags/*:refs/tags/*"],
                        Some(&mut fetch_options),
                        None,
                    )
                    .context("Failed to fetch remote")?;
            }

            return Ok(repo);
        } else {
            fs::remove_dir_all(path.clone())
                .await
                .with_context(|| format!("Failed to clean up corrupted dir at {:?}", path))?;
        }
    }

    let url = url.to_string();
    let path = path.clone();

    let repo = tokio::task::spawn_blocking(move || {
        let repo = RepoBuilder::new()
            .bare(true)
            .clone(&url, &path)
            .context("Failed to clone repository");
        return repo;
    })
    .await??;

    return Ok(repo);
}

async fn link_engine_to_flutter(engine_dir: &PathBuf, flutter_dir: &PathBuf) -> Result<()> {
    let cache_dir = flutter_dir.join("bin").join("cache");
    fs::create_dir_all(&cache_dir).await?;

    // Get the engine hash from the engine directory name
    let engine_hash = engine_dir
        .file_name()
        .and_then(|s| s.to_str())
        .context("Invalid engine directory name")?;

    // Create the three marker files that Flutter expects
    fs::write(cache_dir.join("engine.stamp"), engine_hash).await?;
    fs::write(cache_dir.join("engine-dart-sdk.stamp"), engine_hash).await?;
    fs::write(cache_dir.join("engine.realm"), "").await?;

    // Symlink the entire engine directory as dart-sdk
    // The engine_dir contains bin/, lib/, etc. directly after extraction
    let dart_sdk_link = cache_dir.join("dart-sdk");

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(engine_dir, &dart_sdk_link)?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir;
        symlink_dir(engine_dir, &dart_sdk_link)?;
    }

    Ok(())
}
