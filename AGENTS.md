# AGENTS.md - Coding Agent Guidelines for biu-tui

## Project Overview

**biu-tui** is a terminal-based Bilibili music player built with Rust and Ratatui. It supports high-quality audio playback (Hi-Res/FLAC/192K) and audio extraction downloads.

**Tech Stack:**
- Language: Rust (Edition 2021)
- UI Framework: Ratatui
- Async Runtime: Tokio
- Audio: FFmpeg (ffmpeg-next) + cpal
- HTTP Client: reqwest
- Error Handling: anyhow + thiserror
- Serialization: serde + serde_json

## Build, Lint, and Test Commands

### Building
```bash
# Build in debug mode
cargo build

# Build in release mode (optimized)
cargo build --release

# Check for compilation errors (faster than full build)
cargo check
```

### Linting and Formatting
```bash
# Format code with rustfmt
cargo fmt

# Run clippy linter (must pass before committing)
cargo clippy

# Run clippy with all warnings as errors
cargo clippy -- -D warnings
```

### Testing
```bash
# Run all tests
cargo test

# Run tests with verbose output
cargo test --verbose

# Run a specific test by name
cargo test test_name

# Run tests in a specific module
cargo test module_name::

# Run tests with output (println!)
cargo test -- --nocapture

# Run doc tests
cargo test --doc
```

### Running the Application
```bash
# Run in debug mode
cargo run

# Run in release mode
cargo run --release
```

## Code Style Guidelines

### Formatting
- Use `cargo fmt` before committing (auto-formats according to Rust standards)
- Maximum line length: 100 characters (rustfmt default)
- Use 4 spaces for indentation (no tabs)
- Place opening braces on the same line

### Imports Organization
Imports are organized in this order with blank lines between groups:
1. Standard library (`use std::...`)
2. External crates (`use anyhow::...`, `use tokio::...`)
3. Internal modules (`use crate::...`)
4. Current module items (`use super::...`)

Example:
```rust
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use parking_lot::Mutex;
use ratatui::Terminal;

use crate::api::BilibiliClient;
use crate::audio::AudioPlayer;

use super::decoder::AudioDecoder;
```

### Naming Conventions

**Types (Structs, Enums, Traits):**
- PascalCase: `AudioPlayer`, `LibraryScreen`, `PlayerState`
- Descriptive names: `FavoriteFolder`, not `Folder`

**Functions and Methods:**
- snake_case: `load_data()`, `handle_enter()`, `set_loop_mode()`
- Getters: no `get_` prefix, just `state()`, `volume()`
- Setters: use `set_` prefix: `set_volume()`, `set_loop_mode()`

**Variables:**
- snake_case: `current_tab`, `list_state`, `video_info`
- Short names for obvious contexts: `f` for frame, `e` for error

**Constants:**
- SCREAMING_SNAKE_CASE: `BILIBILI_BASE_URL`

**Module names:**
- snake_case: `audio`, `download`, `storage`

### Error Handling

**Use anyhow for application-level errors:**
```rust
use anyhow::{Context, Result};

pub fn load() -> Result<Self> {
    let path = Self::settings_dir()?.join("settings.json");
    let content = std::fs::read_to_string(&path)
        .context("Failed to read settings file")?;
    Ok(serde_json::from_str(&content)?)
}
```

**Use thiserror for library-style custom errors (if needed):**
```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Network request failed: {0}")]
    Network(String),
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
}
```

**Error handling patterns:**
- Use `?` operator for error propagation
- Add context with `.context()` for better error messages
- Use `anyhow::bail!()` for early returns with errors
- Handle errors gracefully in UI code (don't crash the app)

### Type System

**Enums:**
- Use `#[derive(Debug, Clone, Copy, PartialEq, Eq)]` for simple enums
- Use `#[derive(Debug, Clone, Serialize, Deserialize)]` for data enums

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub loop_mode: LoopMode,
}
```

**Structs:**
- Use `#[derive(Debug)]` for all public structs
- Use `#[derive(Clone)]` when needed for TUI state management
- Use `#[derive(Default)]` for types with sensible defaults

**Serde:**
- Use `#[serde(rename = "...")]` for API field mapping
- Always derive both `Serialize` and `Deserialize` for persisted types

### Async Patterns

**Use Tokio runtime:**
```rust
let rt = tokio::runtime::Runtime::new()?;
rt.block_on(async_function())?;
```

**For async methods:**
```rust
pub async fn get_user_info(&self) -> Result<UserInfo> {
    let response: ApiResponse<UserInfo> = self.get("/x/space/myinfo").await?;
    response.data.context("Failed to get user info")
}
```

### Shared State Management

**Use Arc<Mutex<T>> for shared mutable state:**
```rust
use parking_lot::Mutex;
use std::sync::Arc;

pub struct AudioPlayer {
    state: Arc<Mutex<PlayerState>>,
    volume: Arc<Mutex<f32>>,
}
```

**Access patterns:**
```rust
// Read
let current_state = *self.state.lock();

// Write
*self.state.lock() = PlayerState::Playing;
```

### Module Organization

**Module structure:**
```rust
// src/lib.rs
pub mod api;
pub mod app;
pub mod audio;

// src/api/mod.rs
pub mod auth;
pub mod client;
pub mod types;

pub use auth::QrCodeData;
pub use client::BilibiliClient;
```

**Guidelines:**
- Each module has a `mod.rs` file
- Re-export public items at module level
- Keep implementation details private (use `pub(super)` or private)

### Testing Guidelines

**Test organization:**
- Place tests in the same file as the code being tested
- Use `#[cfg(test)]` module at the bottom of files

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_mode_next() {
        assert_eq!(LoopMode::LoopFolder.next(), LoopMode::LoopOne);
        assert_eq!(LoopMode::LoopOne.next(), LoopMode::NoLoop);
        assert_eq!(LoopMode::NoLoop.next(), LoopMode::LoopFolder);
    }
}
```

**Async tests:**
```rust
#[tokio::test]
async fn test_async_function() {
    let result = async_function().await;
    assert!(result.is_ok());
}
```

**Test naming:**
- Pattern: `test_<function>_<scenario>_<expected_result>`
- Example: `test_parse_settings_valid_json_returns_settings`

### Documentation

**Public items must have doc comments:**
```rust
/// Represents the playback state of the audio player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    /// Player is stopped
    Stopped,
    /// Player is actively playing
    Playing,
    /// Player is paused
    Paused,
}

/// Loads user settings from disk, returning defaults if not found.
pub fn load() -> Result<Self> {
    // ...
}
```

### Project-Specific Patterns

**Screen/Widget rendering:**
```rust
pub fn render(&mut self, f: &mut Frame, area: Rect) {
    // Create widgets
    let list = List::new(items)
        .block(Block::default().title("Title").borders(Borders::ALL));
    
    // Render to frame
    f.render_widget(list, area);
}
```

**API client usage:**
```rust
let client = Arc::new(Mutex::new(BilibiliClient::new()?));
let client_clone = client.clone();
let rt = tokio::runtime::Runtime::new()?;
rt.block_on(async {
    let locked = client_clone.lock();
    locked.get_user_info().await
})?;
```

### Common Pitfalls to Avoid

1. **Don't panic in production code** - Use `Result` and handle errors gracefully
2. **Don't block async runtime** - Use `spawn_blocking` for CPU-intensive work
3. **Don't clone large structs unnecessarily** - Use references when possible
4. **Don't forget to lock mutexes** - Always acquire locks before accessing shared state
5. **Don't ignore errors** - At minimum, log errors with `eprintln!`

### Commit Guidelines

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Run `cargo test` and ensure all tests pass
- Write clear, descriptive commit messages
- Keep commits focused on a single logical change
