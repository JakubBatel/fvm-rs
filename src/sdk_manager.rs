use crate::{utils, config_manager};
use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use git2::{FetchOptions, Repository, build::RepoBuilder};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, io::Cursor, path::PathBuf, sync::OnceLock};
use tokio::{fs, task};
use tracing::{debug, warn};
use zip::ZipArchive;


#[derive(Clone, Debug, Deserialize, Serialize)]
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

// In-memory cache for releases data (compatible with FVM's approach)
static RELEASES_CACHE: OnceLock<FlutterReleases> = OnceLock::new();

/// Parse a version string that may contain a fork alias (e.g., "mycompany/stable")
///
/// Returns (fork_alias, actual_version) if the version contains a fork alias,
/// or (None, version) if it's a regular version string.
fn parse_fork_syntax(version: &str) -> (Option<String>, String) {
    if let Some((alias, ver)) = version.split_once('/') {
        debug!("Parsed fork syntax: alias='{}', version='{}'", alias, ver);
        (Some(alias.to_string()), ver.to_string())
    } else {
        (None, version.to_string())
    }
}

/// Get the Flutter repository URL for a given version
///
/// If the version contains a fork alias (e.g., "mycompany/stable"),
/// looks up the fork URL from global config. Otherwise returns the default URL.
async fn get_flutter_repo_url(version: &str) -> Result<String> {
    let (fork_alias, _actual_version) = parse_fork_syntax(version);

    if let Some(alias) = fork_alias {
        debug!("Looking up fork URL for alias: {}", alias);
        let config = config_manager::GlobalConfig::read().await?;

        if let Some(url) = config.get_fork_url(&alias) {
            debug!("Found fork URL for '{}': {}", alias, url);
            Ok(url)
        } else {
            anyhow::bail!(
                "Fork '{}' not found. Add it with: fvm-rs fork add {} <git-url>",
                alias,
                alias
            );
        }
    } else {
        // Use default URL from config or fallback
        let config = config_manager::GlobalConfig::read().await?;
        Ok(config.get_flutter_url())
    }
}

/// Get the actual version string without fork alias
///
/// For "mycompany/stable" returns "stable"
/// For "3.24.0" returns "3.24.0"
fn strip_fork_alias(version: &str) -> String {
    parse_fork_syntax(version).1
}

/// Get the channel for a given Flutter version
/// Returns the channel name (stable, beta, dev, master) or defaults to "master" if not found
pub async fn get_channel_for_version(version: &str) -> Result<String> {
    debug!("Determining channel for version: {}", version);

    // Strip fork alias if present (e.g., "mycompany/stable" -> "stable")
    let actual_version = strip_fork_alias(version);
    debug!("Actual version (without fork alias): {}", actual_version);

    // Get or fetch releases
    let releases = match RELEASES_CACHE.get() {
        Some(cached) => {
            debug!("Using cached releases data");
            cached
        }
        None => {
            debug!("Fetching releases data (not cached yet)");
            let fetched = list_available_versions().await?;
            // Try to cache it, but if another thread beat us to it, use theirs
            RELEASES_CACHE.get_or_init(|| fetched)
        }
    };

    // Look up the version in the releases
    for release in &releases.releases {
        if release.version == actual_version {
            debug!("Found version {} in channel: {}", actual_version, release.channel);
            return Ok(release.channel.clone());
        }
    }

    // Default to "master" if not found (FVM compatibility - handles custom versions)
    warn!("Version {} not found in releases, defaulting to 'master' channel", actual_version);
    Ok("master".to_string())
}

pub async fn ensure_installed(version: &str) -> Result<()> {
    if !verify_installed(version)? {
        install(version).await?;
    }
    return Ok(());
}

pub async fn list_installed_versions() -> Result<Vec<String>> {
    let flutter_root = utils::flutter_dir()?;
    debug!("Listing installed versions from: {}", flutter_root.display());

    if !flutter_root.exists() {
        debug!("Flutter root directory does not exist yet");
        return Ok(vec![]);
    }

    let mut entries = fs::read_dir(flutter_root).await?;
    let mut versions = vec![];

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if fs::metadata(&path).await?.is_dir() {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                debug!("Found installed version: {}", name);
                versions.push(name.to_string());
            }
        }
    }

    debug!("Found {} installed version(s)", versions.len());
    return Ok(versions);
}

pub async fn list_available_versions() -> Result<FlutterReleases> {
    let platform = std::env::consts::OS;

    let url = format!(
        "https://storage.googleapis.com/flutter_infra_release/releases/releases_{platform}.json"
    );
    debug!("Fetching available Flutter releases from: {}", url);
    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch list of available versions")?
        .error_for_status()?;

    debug!("Parsing releases JSON response");
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
    debug!("Checking for unused engines in: {}", engine_dir.display());

    // If the engine directory doesn't exist, nothing to clean up
    if !engine_dir.exists() {
        debug!("Engine directory does not exist, nothing to clean up");
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
            debug!("Version {} uses engine hash: {}", version, hash);
            used_engines.insert(hash);
        }
    }

    debug!("Found {} engine hash(es) in use", used_engines.len());

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
                debug!("Removing unused engine: {}", hash);
                match fs::remove_dir_all(&path).await {
                    Ok(_) => {
                        debug!("Successfully removed engine: {}", hash);
                        removed_engines.push(hash.to_string());
                    }
                    Err(e) => {
                        warn!("Failed to remove engine {}: {}", hash, e);
                        failed_removals.push((hash.to_string(), e.to_string()));
                    }
                }
            } else {
                debug!("Engine {} is in use, keeping it", hash);
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
    debug!("Uninstalling Flutter version: {}", version);

    if !flutter_dir.exists() {
        debug!("Version {} not found at {}", version, flutter_dir.display());
        return Ok(None);
    }

    // Get the engine hash before deleting the directory
    let engine_hash = get_engine_hash_for_version(version).await?;
    if let Some(hash) = &engine_hash {
        debug!("Version {} uses engine hash: {}", version, hash);
    }

    // Delete the Flutter directory
    debug!("Removing directory: {}", flutter_dir.display());
    fs::remove_dir_all(&flutter_dir).await?;

    // Remove the worktree from git
    let shared_repo_path = utils::shared_flutter_dir()?;
    let worktree_name = format!("fvm-{}", version);
    debug!("Pruning git worktree: {}", worktree_name);

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

    debug!("Successfully uninstalled Flutter version: {}", version);
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
    debug!("Starting installation of Flutter version: {}", version);

    // Get the repository URL (may be a fork)
    let repo_url = get_flutter_repo_url(version).await?;
    debug!("Using Flutter repository: {}", repo_url);

    let engine_hash = fetch_engine_hash(version).await?;
    debug!("Engine hash for version {}: {}", version, engine_hash);

    let engine_dir = utils::shared_engine_hash_dir(&engine_hash)?;
    let flutter_dir = utils::flutter_version_dir(version)?;
    debug!("Engine directory: {}", engine_dir.display());
    debug!("Flutter directory: {}", flutter_dir.display());

    // Get the channel for this version before installation
    let channel = get_channel_for_version(version).await?;
    debug!("Version {} belongs to channel: {}", version, channel);

    debug!("Installing engine and Flutter in parallel");
    let (engine_result, flutter_result) =
        tokio::join!(install_engine(&engine_dir), install_flutter(&flutter_dir, version, &channel, &repo_url),);

    engine_result?;
    flutter_result?;

    debug!("Linking engine to Flutter installation");
    link_engine_to_flutter(&engine_dir, &flutter_dir).await?;

    debug!("Successfully completed installation of Flutter {}", version);
    return Ok(());
}

async fn fetch_engine_hash(version: &str) -> Result<String> {
    // Strip fork alias if present
    let actual_version = strip_fork_alias(version);

    let url = format!(
        "https://raw.githubusercontent.com/flutter/flutter/{}/bin/internal/engine.version",
        actual_version
    );
    debug!("Fetching engine hash from: {}", url);

    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch engine hash")?
        .error_for_status()?;

    let hash = response
        .text()
        .await
        .context("Could not read engine.version")?
        .trim()
        .to_string();

    debug!("Fetched engine hash: {}", hash);
    return Ok(hash);
}

async fn install_engine(engine_dir: &PathBuf) -> Result<()> {
    if engine_dir.exists() {
        debug!("Engine already cached at: {}", engine_dir.display());
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
    debug!("Installing engine {} for {}-{}", engine_hash, platform, arch);

    let url = format!(
        "https://storage.googleapis.com/flutter_infra_release/flutter/{}/dart-sdk-{}-{}.zip",
        engine_hash, platform, arch
    );
    debug!("Downloading engine from: {}", url);

    let response = reqwest::get(&url)
        .await
        .context("Failed to fetch engine zip")?
        .error_for_status()
        .context("Failed to fetch engine zip")?;

    debug!("Downloading engine zip archive");
    let bytes = response
        .bytes()
        .await
        .context("Failed to read engine zip")?;

    debug!("Extracting engine archive ({} bytes)", bytes.len());
    let cursor = Cursor::new(bytes);
    let mut archive = ZipArchive::new(cursor)?;

    debug!("Creating engine directory: {}", engine_dir.display());
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

    debug!("Successfully installed engine to: {}", engine_dir.display());
    return Ok(());
}

async fn install_flutter(version_dir: &PathBuf, version: &str, channel: &str, repo_url: &str) -> Result<()> {
    let shared_dir = utils::shared_flutter_dir()?;
    debug!("Setting up Flutter repository from: {}", repo_url);

    let repo = ensure_shared_repo(repo_url, &shared_dir).await?;

    let parent_dir = version_dir.parent().unwrap();
    debug!("Creating parent directory: {}", parent_dir.display());
    fs::create_dir_all(parent_dir).await?;

    debug!("Creating git worktree for version: {} (channel: {})", version, channel);
    debug!("Worktree will be created at: {}", version_dir.display());

    let version_dir_clone = version_dir.clone();
    let version_string = version.to_string();
    let channel_string = channel.to_string();

    task::spawn_blocking(move || {
        let worktree_name = format!("fvm-{}", version_string);
        debug!("Creating worktree '{}' using channel branch '{}'", worktree_name, channel_string);

        // Find the channel branch reference (e.g., "refs/heads/stable")
        let branch_ref_name = format!("refs/heads/{}", channel_string);
        debug!("Finding channel branch reference: {}", branch_ref_name);
        let branch_ref = repo.find_reference(&branch_ref_name)
            .context("Failed to find channel branch")?;

        // Create the worktree using the channel branch
        // This makes Flutter doctor recognize the correct channel
        let mut opts = git2::WorktreeAddOptions::new();
        opts.reference(Some(&branch_ref));

        let worktree = repo
            .worktree(&worktree_name, &version_dir_clone, Some(&opts))
            .context("Failed to create worktree")?;

        debug!("Opening worktree repository at: {}", worktree.path().display());
        let worktree_repo =
            Repository::open(worktree.path()).context("Failed to open worktree repository")?;

        // Find the specific version tag
        let commit_ref = format!("refs/tags/{}", version_string);
        debug!("Finding version tag: {}", commit_ref);

        let commit = worktree_repo
            .find_reference(&commit_ref)?
            .peel_to_commit()?;

        // Reset to the specific version while staying on the channel branch
        debug!("Resetting {} branch to commit {} (version {})", channel_string, commit.id(), version_string);
        worktree_repo.reset(commit.as_object(), git2::ResetType::Hard, None)?;

        // Configure the branch to track origin/{channel}
        let mut config = worktree_repo.config()?;
        let branch_remote_key = format!("branch.{}.remote", channel_string);
        let branch_merge_key = format!("branch.{}.merge", channel_string);

        debug!("Configuring branch '{}' to track 'origin/{}'", channel_string, channel_string);
        config.set_str(&branch_remote_key, "origin")
            .context("Failed to set branch remote")?;
        config.set_str(&branch_merge_key, &format!("refs/heads/{}", channel_string))
            .context("Failed to set branch merge")?;

        debug!("Successfully set up Flutter version {} on channel {} with upstream tracking", version_string, channel_string);
        return Ok::<_, anyhow::Error>(());
    })
    .await??;

    debug!("Successfully set up Flutter at: {}", version_dir.display());
    return Ok(());
}

async fn ensure_shared_repo(url: &str, path: &PathBuf) -> Result<git2::Repository> {
    if path.exists() {
        debug!("Shared repository already exists at: {}", path.display());
        let repo_result = Repository::open_bare(path.clone());
        if let Ok(repo) = repo_result {
            {
                // Ensure advice.detachedHead is disabled to suppress warnings
                debug!("Configuring git advice.detachedHead=false");
                let mut config = repo.config()?;
                config.set_bool("advice.detachedHead", false)?;

                debug!("Fetching updates from remote: {}", url);
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

                debug!("Successfully fetched updates from remote");
            }

            return Ok(repo);
        } else {
            warn!("Corrupted repository found at {}, cleaning up", path.display());
            fs::remove_dir_all(path.clone())
                .await
                .with_context(|| format!("Failed to clean up corrupted dir at {:?}", path))?;
        }
    }

    debug!("Cloning shared bare repository from: {}", url);
    debug!("Clone destination: {}", path.display());

    let url = url.to_string();
    let path_clone = path.clone();

    let repo = tokio::task::spawn_blocking(move || {
        let repo = RepoBuilder::new()
            .bare(true)
            .clone(&url, &path_clone)
            .context("Failed to clone repository")?;

        // Configure advice.detachedHead=false to suppress warnings
        debug!("Configuring git advice.detachedHead=false");
        let mut config = repo.config()?;
        config.set_bool("advice.detachedHead", false)?;

        Ok::<_, anyhow::Error>(repo)
    })
    .await??;

    debug!("Successfully cloned shared repository to: {}", path.display());
    return Ok(repo);
}

async fn link_engine_to_flutter(engine_dir: &PathBuf, flutter_dir: &PathBuf) -> Result<()> {
    let cache_dir = flutter_dir.join("bin").join("cache");
    debug!("Creating cache directory: {}", cache_dir.display());
    fs::create_dir_all(&cache_dir).await?;

    // Get the engine hash from the engine directory name
    let engine_hash = engine_dir
        .file_name()
        .and_then(|s| s.to_str())
        .context("Invalid engine directory name")?;

    debug!("Creating engine marker files for hash: {}", engine_hash);
    // Create the three marker files that Flutter expects
    fs::write(cache_dir.join("engine.stamp"), engine_hash).await?;
    fs::write(cache_dir.join("engine-dart-sdk.stamp"), engine_hash).await?;
    fs::write(cache_dir.join("engine.realm"), "").await?;

    // Symlink the entire engine directory as dart-sdk
    // The engine_dir contains bin/, lib/, etc. directly after extraction
    let dart_sdk_link = cache_dir.join("dart-sdk");
    debug!("Creating symlink: {} -> {}", dart_sdk_link.display(), engine_dir.display());

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

    debug!("Successfully linked engine to Flutter installation");
    Ok(())
}

/// Set a Flutter version as the global default
///
/// Creates a symlink at ~/.fvm-rs/default pointing to the specified version.
/// The version must be installed first.
pub async fn set_global_version(version: &str) -> Result<()> {
    let flutter_version_dir = utils::flutter_version_dir(version)?;

    // Verify the version is installed
    if !flutter_version_dir.exists() {
        anyhow::bail!(
            "Flutter version {} is not installed. Run 'fvm-rs install {}' first.",
            version,
            version
        );
    }

    let global_link = utils::get_global_link_path()?;

    // Remove existing symlink if it exists
    if global_link.exists() || global_link.symlink_metadata().is_ok() {
        debug!("Removing existing global symlink: {}", global_link.display());
        fs::remove_file(&global_link).await
            .context("Failed to remove existing global symlink")?;
    }

    debug!("Creating global symlink: {} -> {}",
           global_link.display(),
           flutter_version_dir.display());

    // Create the symlink
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        tokio::task::spawn_blocking(move || {
            symlink(&flutter_version_dir, &global_link)
        })
        .await?
        .context("Failed to create global symlink")?;
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_dir;
        tokio::task::spawn_blocking(move || {
            symlink_dir(&flutter_version_dir, &global_link)
        })
        .await?
        .context("Failed to create global symlink")?;
    }

    debug!("Successfully set global version to: {}", version);
    Ok(())
}

/// Unset the global Flutter version
///
/// Removes the symlink at ~/.fvm-rs/default.
/// Returns Ok(false) if no global version was set, Ok(true) if it was removed.
pub async fn unset_global_version() -> Result<bool> {
    let global_link = utils::get_global_link_path()?;

    // Check if symlink exists (using symlink_metadata to avoid following the link)
    if global_link.symlink_metadata().is_ok() {
        debug!("Removing global symlink: {}", global_link.display());
        fs::remove_file(&global_link).await
            .context("Failed to remove global symlink")?;

        debug!("Successfully removed global version");
        Ok(true)
    } else {
        debug!("No global symlink found at: {}", global_link.display());
        Ok(false)
    }
}

/// Get the currently set global version
///
/// Returns the version name if a global version is set, or None.
pub async fn get_global_version() -> Result<Option<String>> {
    let global_link = utils::get_global_link_path()?;

    // Check if symlink exists
    if let Ok(target) = fs::read_link(&global_link).await {
        debug!("Found global version symlink: {} -> {}",
               global_link.display(),
               target.display());

        // Extract version name from target path
        // Target format: ~/.fvm-rs/flutter/{version}
        if let Some(version) = target.file_name() {
            let version_str = version.to_string_lossy().to_string();
            debug!("Global version: {}", version_str);
            return Ok(Some(version_str));
        }
    }

    debug!("No global version configured");
    Ok(None)
}
