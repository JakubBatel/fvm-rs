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

pub async fn uninstall(version: &str) -> Result<(), anyhow::Error> {
    let flutter_dir = utils::flutter_version_dir(version)?;
    if flutter_dir.exists() {
        fs::remove_dir_all(flutter_dir).await?;
    }
    return Ok(());
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

    println!("Resolved engine hash: {}", engine_hash);

    let engine_dir = utils::shared_engine_hash_dir(&engine_hash)?;
    let flutter_dir = utils::flutter_version_dir(version)?;

    let (engine_result, flutter_result) =
        tokio::join!(install_engine(&engine_dir), install_flutter(&flutter_dir),);

    engine_result?;
    flutter_result?;

    println!("Linking engine to flutter");
    link_engine_to_flutter(&engine_dir, &flutter_dir).await?;

    return Ok(());
}

async fn fetch_engine_hash(version: &str) -> Result<String> {
    println!("Fetching engine hash for version: {}", version);

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
    println!("Shared repo obtained ({:?})", repo.path());

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
