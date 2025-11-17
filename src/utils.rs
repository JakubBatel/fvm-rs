use anyhow::{Context, Result};
use dirs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tracing::debug;

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

/// Execute a command with modified PATH to use a specific Flutter version
///
/// This prepends the Flutter bin directories to PATH and executes the command
/// with live output (inheriting stdio).
///
/// Returns the exit code of the subprocess.
pub fn execute_with_flutter_path(
    command: &str,
    args: &[String],
    flutter_path: &PathBuf,
) -> Result<i32> {
    // Construct bin paths to prepend to PATH
    let flutter_bin = flutter_path.join("bin");
    let dart_bin = flutter_path.join("bin").join("cache").join("dart-sdk").join("bin");

    debug!("Executing {} with Flutter at: {}", command, flutter_path.display());
    debug!("Flutter bin: {}", flutter_bin.display());
    debug!("Dart bin: {}", dart_bin.display());

    // Get current PATH
    let current_path = std::env::var("PATH").unwrap_or_default();

    // Prepend Flutter paths to PATH
    let separator = if cfg!(windows) { ";" } else { ":" };
    let new_path = format!(
        "{}{}{}{}{}",
        flutter_bin.display(),
        separator,
        dart_bin.display(),
        separator,
        current_path
    );

    debug!("Modified PATH: {}", new_path);

    // Execute command with modified environment
    let mut cmd = Command::new(command);
    cmd.args(args)
        .env("PATH", new_path)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    debug!("Running: {} {}", command, args.join(" "));

    let status = cmd.status()
        .context(format!("Failed to execute {}", command))?;

    let exit_code = status.code().unwrap_or(1);
    debug!("Command exited with code: {}", exit_code);

    Ok(exit_code)
}

/// Execute a command using system PATH (fallback when no version is configured)
///
/// Returns the exit code of the subprocess.
pub fn execute_with_system_path(command: &str, args: &[String]) -> Result<i32> {
    debug!("Executing {} using system PATH", command);
    debug!("Running: {} {}", command, args.join(" "));

    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = cmd.status()
        .context(format!("Failed to execute {}", command))?;

    let exit_code = status.code().unwrap_or(1);
    debug!("Command exited with code: {}", exit_code);

    Ok(exit_code)
}
