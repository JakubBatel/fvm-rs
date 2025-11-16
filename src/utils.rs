use anyhow::{Context, Result};
use dirs;
use std::path::PathBuf;

pub fn fvm_rs_root_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("Could not find home directory")?
        .join(".fvm-rs"))
}

pub fn shared_dir() -> Result<PathBuf> {
    Ok(fvm_rs_root_dir()?.join("shared"))
}

pub fn shared_flutter_dir() -> Result<PathBuf> {
    Ok(shared_dir()?.join("flutter"))
}

pub fn shared_engine_dir() -> Result<PathBuf> {
    Ok(shared_dir()?.join("engine"))
}

pub fn flutter_dir() -> Result<PathBuf> {
    Ok(fvm_rs_root_dir()?.join("flutter"))
}

pub fn flutter_version_dir(version: &str) -> Result<PathBuf> {
    Ok(flutter_dir()?.join(version))
}

pub fn shared_engine_hash_dir(hash: &str) -> Result<PathBuf> {
    Ok(shared_dir()?.join("engine").join(hash))
}
