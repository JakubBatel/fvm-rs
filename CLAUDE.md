# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

fvm-rs is a Rust reimplementation of FVM (Flutter Version Manager), designed to be a drop-in replacement with significant performance improvements. It combines the speed of Rust with smart optimizations inspired by Puro, particularly git worktrees for efficient version management.

**Performance improvements over original FVM:**
1. **Rust performance** - Native code execution instead of Dart VM
2. **Git worktrees** - Single bare repository with worktrees per version (inspired by Puro), avoiding redundant git history downloads

The goal is feature parity with the original FVM (located in `fvm/` directory) while being significantly faster.

## Core Architecture

### Storage Structure (`~/.fvm-rs/`)

```
~/.fvm-rs/
├── shared/
│   ├── flutter/        # Bare git repo shared across all versions
│   └── engine/{hash}/  # Shared Dart SDK engines by hash (deduplication)
└── flutter/{version}/  # Per-version installations (git worktrees)
    └── bin/cache/
        ├── dart-sdk -> symlink to shared/engine/{hash}/
        ├── engine.stamp
        ├── engine-dart-sdk.stamp
        └── engine.realm
```

### Key Optimization: Git Worktrees

Unlike FVM which clones Flutter separately for each version, fvm-rs:
1. Maintains one bare repository in `shared/flutter/`
2. Creates git worktrees for each version in `flutter/{version}/`
3. Shares git objects across all versions, dramatically reducing disk usage and download time
4. Fetches updates once and all worktrees benefit

### Engine Deduplication

Dart SDK engines are downloaded once per hash to `~/.fvm-rs/shared/engine/{hash}/` and symlinked into each Flutter installation's `bin/cache/dart-sdk` directory. Multiple Flutter versions often share the same engine hash, avoiding redundant downloads.

**Critical implementation details:**
- Engines are symlinked as `flutter/bin/cache/dart-sdk` (entire directory symlink, not individual files)
- Three marker files must be created in `bin/cache/`: `engine.stamp`, `engine-dart-sdk.stamp`, `engine.realm`
- These marker files prevent Flutter from attempting to re-download the engine

### Parallel Operations

Engine download and Flutter repository setup happen concurrently (see `sdk_manager::install` line 158-162).

## Module Organization

- `main.rs` - CLI entry point with clap for argument parsing
- `sdk_manager.rs` - Core installation logic: git operations, engine downloads, worktree management
- `config_manager.rs` - Configuration (currently stub, needs implementation for project-level config)
- `utils.rs` - Path resolution for fvm-rs directory structure
- `commands/` - Command implementations mirroring FVM's API

## Architecture: Separation of Logic and Presentation

**IMPORTANT: This project follows a strict separation between business logic and presentation:**

### Logic Layer (`sdk_manager.rs`, `config_manager.rs`)
- **NEVER** use `println!`, `eprintln!`, or any other user-facing output
- **ALWAYS** return `Result<T>` types - let errors propagate upward
- Return structured data that commands can present (e.g., `EngineCleanupResult` instead of just a count)
- Focus solely on core functionality: file operations, git operations, network requests, etc.

### Presentation Layer (`src/commands/*.rs`)
- **ALWAYS** handle all user-facing output: success messages, error messages, progress indicators
- Format errors in a user-friendly way
- Display progress and status updates
- Use checkmarks (✓) and crosses (✗) for visual feedback
- Handle `Result` types from the logic layer and present them appropriately

### Example Pattern

**Bad (mixing logic and presentation):**
```rust
// In sdk_manager.rs
pub async fn cleanup_unused_engines() -> Result<usize> {
    // ...
    println!("Removed engine {}", hash);  // ❌ NO!
    // ...
}
```

**Good (separated):**
```rust
// In sdk_manager.rs
pub struct EngineCleanupResult {
    pub removed_engines: Vec<String>,
    pub failed_removals: Vec<(String, String)>,
}

pub async fn cleanup_unused_engines() -> Result<EngineCleanupResult> {
    // ... pure logic, no printing ...
    Ok(EngineCleanupResult { removed_engines, failed_removals })
}

// In src/commands/rm.rs
match sdk_manager::cleanup_unused_engines().await {
    Ok(result) => {
        for hash in &result.removed_engines {
            println!("✓ Removed unused engine: {}", hash);
        }
        for (hash, error) in &result.failed_removals {
            eprintln!("✗ Failed to remove engine {}: {}", hash, error);
        }
    }
    Err(e) => eprintln!("Warning: Engine cleanup failed: {}", e),
}
```

This separation ensures:
- Logic modules are testable without capturing stdout
- Presentation can be changed without touching core logic
- Error messages can be localized or customized at the command level
- The same logic functions can be used by different commands with different presentation needs

## Logging and Verbose Output

**Framework:** This project uses the `tracing` crate for structured logging.

### Global Verbose Flag

The CLI supports a global `--verbose` / `-v` flag that enables debug-level logging:

```bash
fvm-rs -v install 3.24.0        # Verbose install
fvm-rs --verbose use stable     # Verbose use command
fvm-rs install 3.24.0           # Normal (quiet) operation
```

**Important:** The verbose flag is **global** and must appear **before** the subcommand.

### Logging Guidelines

#### Logic Layer (sdk_manager.rs, config_manager.rs)
Use `tracing` macros for internal operations - never `println!` or `eprintln!`:

- `debug!()` - Detailed diagnostic information (only shown with `-v`):
  - Git operations (clone, fetch, checkout, worktree creation)
  - File system operations (mkdir, symlink, copy, remove)
  - Network operations (HTTP requests, download URLs)
  - Engine/cache details (hash calculations, cache hits/misses, deduplication)
- `warn!()` - Recoverable issues that don't fail the operation
- `error!()` - Errors before returning `Err()`

**Example:**
```rust
// In sdk_manager.rs
pub async fn install_flutter(version: &str) -> Result<PathBuf> {
    debug!("Installing Flutter version: {}", version);

    let flutter_dir = get_flutter_path(version);
    debug!("Target directory: {}", flutter_dir.display());

    tokio::fs::create_dir_all(&flutter_dir).await?;
    debug!("Created directory: {}", flutter_dir.display());

    // ... rest of implementation
}
```

#### Presentation Layer (src/commands/*.rs)
- Use `info!()` for high-level progress steps (shown even without `-v`)
- Use `println!()` for normal command output (version lists, tables, etc.)
- Use `eprintln!()` for user-facing error messages

**Example:**
```rust
// In src/commands/install.rs
pub async fn run(args: InstallArgs) -> Result<()> {
    info!("Starting installation of Flutter {}", args.version);

    match sdk_manager::install(&args.version).await {
        Ok(path) => {
            println!("✓ Flutter {} installed successfully", args.version);
            debug!("Installed at: {}", path.display());
            Ok(())
        }
        Err(e) => {
            eprintln!("✗ Installation failed: {}", e);
            Err(e)
        }
    }
}
```

### Log Levels and Format

The logging output uses a compact, readable format:
```
HH:MM:SS L message
```

Where:
- `HH:MM:SS` - Local time in 24-hour format (gray)
- `L` - Single-letter log level (colored):
  - `E` - Error (red)
  - `W` - Warning (yellow)
  - `I` - Info (green)
  - `D` - Debug (blue)
  - `T` - Trace (purple)

**Example output:**
```
23:09:45 I Listing installed Flutter SDK versions
23:09:45 D Listing installed versions from: /Users/jakub/.fvm-rs/flutter
23:09:45 D Found installed version: 3.38.1
```

**Log levels:**
- **Normal mode** (default): Only `WARN` and `ERROR` from logic layer, plus all command output
- **Verbose mode** (`-v`): All `DEBUG` and above, showing detailed operations

### What Gets Logged in Verbose Mode

1. **Git operations:**
   - Repository initialization/cloning
   - Fetch operations and progress
   - Worktree creation and checkout
   - Branch/tag resolution

2. **File system operations:**
   - Directory creation
   - Symlink operations (creation, target paths)
   - File/directory removal
   - Path resolutions

3. **Network operations:**
   - Engine download URLs
   - HTTP request/response details
   - Download progress

4. **Engine/cache operations:**
   - Engine hash calculations
   - Cache hit/miss decisions
   - Deduplication logic
   - Shared engine reuse

## Development Commands

**Build:**
```bash
cargo build
cargo build --release
```

**Run during development:**
```bash
cargo run -- releases --channel stable
cargo run -- use 3.24.0
cargo run -- ls
cargo run -- rm 3.24.0
```

**Run tests:**
```bash
cargo test
```

## Implementation Notes

### Adding Commands

To maintain FVM compatibility:
1. Check the original FVM command in `fvm/lib/src/commands/`
2. Create equivalent in `src/commands/`
3. Define Args struct with clap derives
4. Implement async `run()` function
5. Add to `Commands` enum in `main.rs`

### Platform Handling

- **Git operations**: Use `git2` crate, wrap in `spawn_blocking` for long operations (git2 is CPU-bound)
- **Symlinks**: Different APIs for Unix vs Windows (see `link_engine_to_flutter` in `sdk_manager.rs:347-380`)
- **Engine platform naming**: macOS → "darwin", handle arm64/aarch64 variants (see `sdk_manager.rs:196-204`)
- **Flutter executables**: `flutter.bat` on Windows, `flutter` on Unix

### Async Patterns

- Use `tokio::join!` for independent parallel operations
- Wrap `git2` operations in `task::spawn_blocking` (they're synchronous/CPU-bound)
- Use `tokio::fs` for async file operations

## Current Implementation Status

### Implemented Commands
- `install <version>` - Downloads and caches a Flutter SDK version
- `use <version>` - Sets Flutter SDK version for current project (creates `.fvm/fvm_config.json`)
- `ls` - List installed versions
- `releases --channel <channel>` - Show available releases with pretty tables
- `rm <version>` - Remove installed version

### Engine Linking - FIXED ✓
The engine linking now works correctly:
- Engines are cached in `~/.fvm-rs/shared/engine/{hash}/` (deduplication enabled)
- Proper symlink structure: `flutter/bin/cache/dart-sdk -> shared/engine/{hash}/`
- Marker files created: `engine.stamp`, `engine-dart-sdk.stamp`, `engine.realm`
- Flutter no longer attempts to re-download the engine

### Missing for FVM Parity
- Global version setting and persistence
- `flutter` and `dart` passthrough commands
- `exec` command for running commands in FVM context
- `doctor` command
- `config` command for global settings
- Flavor support (project variants)
- Fork/custom Flutter repository support
- `--skip-pub-get` and `--skip-setup` flags for `use` command

### Known TODOs
- `config_manager.rs` is mostly a stub
- No global version configuration yet
- `use` command should optionally run `flutter pub get`

## Reference Implementation

The `fvm/` directory contains the original Dart implementation. Refer to it for:
- Command behavior and options
- Configuration file format (`.fvm/fvm_config.json`)
- Error messages and user experience
- Global config structure

The `puro/` directory shows the git worktree optimization pattern that inspired this implementation.
