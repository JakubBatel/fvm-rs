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
