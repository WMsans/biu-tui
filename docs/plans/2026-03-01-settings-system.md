# Settings System Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a settings system with dedicated settings screen for volume control and loop behavior.

**Architecture:** Generic `SettingItem` trait with typed setting implementations. Settings stored in separate `settings.json`, auto-saved on change. Settings screen uses j/k navigation and h/l for value adjustment.

**Tech Stack:** Rust, serde for serialization, existing ratatui TUI framework

---

### Task 1: Create Settings Module with Core Types

**Files:**
- Create: `src/storage/settings.rs`
- Modify: `src/storage/mod.rs`

**Step 1: Create settings.rs with LoopMode enum and Settings struct**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    LoopFolder,
    LoopOne,
    NoLoop,
}

impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            LoopMode::LoopFolder => LoopMode::LoopOne,
            LoopMode::LoopOne => LoopMode::NoLoop,
            LoopMode::NoLoop => LoopMode::LoopFolder,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LoopMode::LoopFolder => LoopMode::NoLoop,
            LoopMode::LoopOne => LoopMode::LoopFolder,
            LoopMode::NoLoop => LoopMode::LoopOne,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            LoopMode::LoopFolder => "Loop Folder",
            LoopMode::LoopOne => "Loop One",
            LoopMode::NoLoop => "No Loop",
        }
    }
}

impl Default for LoopMode {
    fn default() -> Self {
        LoopMode::NoLoop
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub loop_mode: LoopMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 100,
            loop_mode: LoopMode::default(),
        }
    }
}

impl Settings {
    pub fn settings_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    pub fn load() -> Result<Self> {
        let path = Self::settings_dir()?.join("settings.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            let settings = Self::default();
            settings.save()?;
            Ok(settings)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::settings_dir()?.join("settings.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 5).min(100);
        let _ = self.save();
    }

    pub fn volume_down(&mut self) {
        self.volume = self.volume.saturating_sub(5);
        let _ = self.save();
    }

    pub fn next_loop_mode(&mut self) {
        self.loop_mode = self.loop_mode.next();
        let _ = self.save();
    }

    pub fn prev_loop_mode(&mut self) {
        self.loop_mode = self.loop_mode.prev();
        let _ = self.save();
    }

    pub fn volume_float(&self) -> f32 {
        self.volume as f32 / 100.0
    }
}
```

**Step 2: Modify src/storage/mod.rs to export settings**

Add at the end of the file:

```rust
pub mod settings;
pub use settings::{LoopMode, Settings};
```

**Step 3: Commit**

```bash
git add src/storage/settings.rs src/storage/mod.rs
git commit -m "feat: add Settings module with volume and loop mode"
```

---

### Task 2: Create Settings Screen

**Files:**
- Create: `src/screens/settings.rs`
- Modify: `src/screens/mod.rs`

**Step 1: Create settings.rs with SettingsScreen**

```rust
use crate::storage::Settings;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    Volume,
    LoopMode,
}

impl SettingItem {
    pub fn next(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::Volume,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::Volume,
        }
    }
}

pub struct SettingsScreen {
    pub settings: Settings,
    pub list_state: ListState,
    pub selected_item: SettingItem,
}

impl SettingsScreen {
    pub fn new(settings: Settings) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            settings,
            list_state,
            selected_item: SettingItem::Volume,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(2),
            ])
            .split(area);

        let title = Paragraph::new("Settings")
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        let items = self.build_items();
        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let help = Paragraph::new("[j/k] Navigate  [h/l] Adjust  [Esc/s] Back")
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    fn build_items(&self) -> Vec<ListItem> {
        let volume_text = format!(
            "Volume        {}  {:3}%",
            self.format_volume_bar(),
            self.settings.volume
        );
        let loop_text = format!(
            "Loop Mode     {}",
            self.settings.loop_mode.display_name()
        );

        vec![ListItem::new(volume_text), ListItem::new(loop_text)]
    }

    fn format_volume_bar(&self) -> String {
        let bar_width = 20;
        let filled = (bar_width as f32 * (self.settings.volume as f32 / 100.0)) as usize;
        let filled = filled.min(bar_width);
        let empty = bar_width - filled;
        
        let filled_chars: String = std::iter::repeat('█').take(filled).collect();
        let empty_chars: String = std::iter::repeat('░').take(empty).collect();
        format!("{}{}", filled_chars, empty_chars)
    }

    pub fn next_item(&mut self) {
        self.selected_item = self.selected_item.next();
        self.list_state.select(Some(self.selected_item as usize));
    }

    pub fn prev_item(&mut self) {
        self.selected_item = self.selected_item.prev();
        self.list_state.select(Some(self.selected_item as usize));
    }

    pub fn adjust_up(&mut self) {
        match self.selected_item {
            SettingItem::Volume => self.settings.volume_up(),
            SettingItem::LoopMode => self.settings.next_loop_mode(),
        }
    }

    pub fn adjust_down(&mut self) {
        match self.selected_item {
            SettingItem::Volume => self.settings.volume_down(),
            SettingItem::LoopMode => self.settings.prev_loop_mode(),
        }
    }
}
```

**Step 2: Modify src/screens/mod.rs to export SettingsScreen**

Add at the end:

```rust
pub mod settings;
pub use settings::{SettingItem, SettingsScreen};
```

**Step 3: Commit**

```bash
git add src/screens/settings.rs src/screens/mod.rs
git commit -m "feat: add SettingsScreen with volume bar and loop mode display"
```

---

### Task 3: Integrate Settings into App

**Files:**
- Modify: `src/app.rs`

**Step 1: Add Settings to App struct and Screen enum**

In `src/app.rs`, add import at the top:

```rust
use crate::screens::SettingsScreen;
use crate::storage::Settings;
```

Add to the `Screen` enum:

```rust
pub enum Screen {
    Login(LoginScreen),
    Library(LibraryScreen),
    Settings(SettingsScreen),
}
```

Add to the `App` struct:

```rust
pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
    screen: Screen,
    client: Arc<Mutex<BilibiliClient>>,
    player: Option<AudioPlayer>,
    downloader: Option<DownloadManager>,
    config: Config,
    last_qr_poll: Option<Instant>,
    settings: Settings,
}
```

**Step 2: Initialize settings in App::new() and apply volume to player**

In `App::new()`, after loading config:

```rust
let settings = Settings::load().unwrap_or_default();
```

Add `settings` to the return struct:

```rust
Ok(Self {
    terminal,
    running: true,
    screen,
    client,
    player: None,
    downloader: None,
    config,
    last_qr_poll: None,
    settings,
})
```

**Step 3: Add settings screen rendering in run()**

In the `run()` method, update the render match:

```rust
match &self.screen {
    Screen::Login(login) => login.render(f, area),
    Screen::Library(library) => {
        let mut lib = library.clone();
        lib.render(f, area, self.player.as_ref());
    }
    Screen::Settings(settings_screen) => {
        let mut s = settings_screen.clone();
        s.render(f, area);
    }
}
```

**Step 4: Add key handling for settings screen**

In `handle_key()`, add a new match arm for `Screen::Settings`:

```rust
Screen::Settings(settings_screen) => match code {
    KeyCode::Char('q') => self.running = false,
    KeyCode::Char('s') | KeyCode::Esc => {
        self.screen = Screen::Library(LibraryScreen::new());
        let rt = tokio::runtime::Runtime::new()?;
        if let Err(e) = rt.block_on(self.load_library_data()) {
            eprintln!("Failed to load library data: {}", e);
        }
    }
    KeyCode::Char('j') | KeyCode::Down => settings_screen.next_item(),
    KeyCode::Char('k') | KeyCode::Up => settings_screen.prev_item(),
    KeyCode::Char('l') | KeyCode::Right => {
        settings_screen.adjust_up();
        self.apply_volume();
    }
    KeyCode::Char('h') | KeyCode::Left => {
        settings_screen.adjust_down();
        self.apply_volume();
    }
    _ => {}
},
```

**Step 5: Add 's' key to Library screen handler**

In the `Screen::Library` match arm, add:

```rust
KeyCode::Char('s') => {
    let settings_screen = SettingsScreen::new(self.settings.clone());
    self.screen = Screen::Settings(settings_screen);
}
```

**Step 6: Add helper methods**

Add these methods to `App`:

```rust
fn load_library_data(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(library.load_data(self.client.clone()))?;
    }
    Ok(())
}

fn apply_volume(&mut self) {
    if let Some(player) = &self.player {
        player.set_volume(self.settings.volume_float());
    }
}
```

**Step 7: Apply volume when player is created**

In any place where `AudioPlayer::new()` is called, immediately apply volume:

```rust
*player = Some(AudioPlayer::new()?);
if let Some(p) = player {
    p.set_volume(self.settings.volume_float());
}
```

**Step 8: Commit**

```bash
git add src/app.rs
git commit -m "feat: integrate Settings into App with 's' key access"
```

---

### Task 4: Implement Loop Behavior in Library Playback

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add settings field to LibraryScreen**

In `LibraryScreen` struct, add:

```rust
pub struct LibraryScreen {
    // ... existing fields ...
    pub loop_mode: crate::storage::LoopMode,
}
```

Initialize in `new()`:

```rust
impl LibraryScreen {
    pub fn new() -> Self {
        Self {
            // ... existing fields ...
            loop_mode: crate::storage::LoopMode::default(),
        }
    }
}
```

**Step 2: Add method to set loop mode from settings**

```rust
pub fn set_loop_mode(&mut self, mode: crate::storage::LoopMode) {
    self.loop_mode = mode;
}
```

**Step 3: Add method to handle song end and return next action**

```rust
pub fn get_next_action(&self) -> Option<NextAction> {
    if self.resources.is_empty() {
        return None;
    }

    let current_idx = self.list_state.selected()?;
    
    match self.loop_mode {
        LoopMode::LoopOne => Some(NextAction::ReplayCurrent),
        LoopMode::NoLoop => {
            if current_idx + 1 < self.resources.len() {
                Some(NextAction::PlayNext(current_idx + 1))
            } else {
                None
            }
        }
        LoopMode::LoopFolder => {
            let next_idx = if current_idx + 1 < self.resources.len() {
                current_idx + 1
            } else {
                0
            };
            Some(NextAction::PlayNext(next_idx))
        }
    }
}

pub enum NextAction {
    ReplayCurrent,
    PlayNext(usize),
}
```

**Step 4: Import LoopMode at the top of the file**

```rust
use crate::storage::LoopMode;
```

**Step 5: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add loop behavior logic to LibraryScreen"
```

---

### Task 5: Add Loop Mode to Help Bar

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Update help bar to include 's' for settings**

In `render()`, update the help text:

```rust
let help = Paragraph::new("[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [Tab] Switch")
    .block(Block::default().borders(Borders::TOP));
f.render_widget(help, chunks[5]);
```

**Step 2: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add 's' key to help bar for settings access"
```

---

### Task 6: Wire Up Loop Mode in App

**Files:**
- Modify: `src/app.rs`

**Step 1: Pass loop mode to LibraryScreen when loading**

Update the places where `LibraryScreen` is used. When switching back from settings:

```rust
KeyCode::Char('s') | KeyCode::Esc => {
    let mut library = LibraryScreen::new();
    library.set_loop_mode(self.settings.loop_mode);
    if let Err(e) = self.load_library_data_into(&mut library) {
        eprintln!("Failed to load library data: {}", e);
    }
    self.screen = Screen::Library(library);
}
```

**Step 2: Add helper to load data into existing library**

```rust
fn load_library_data_into(&mut self, library: &mut LibraryScreen) -> Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(library.load_data(self.client.clone()))?;
    Ok(())
}
```

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: pass loop mode from settings to LibraryScreen"
```

---

### Task 7: Test the Implementation

**Step 1: Build the project**

```bash
cargo build
```

Expected: Build succeeds with no errors

**Step 2: Run the application**

```bash
cargo run
```

Manual testing checklist:
- [ ] Press `s` from library screen → settings screen opens
- [ ] `j/k` navigates between Volume and Loop Mode
- [ ] `h/l` adjusts volume (bar changes, percentage changes)
- [ ] `h/l` cycles loop mode (Loop Folder → Loop One → No Loop)
- [ ] Press `Esc` or `s` → returns to library
- [ ] Settings persist after app restart (check `~/.config/biu-tui/settings.json`)
- [ ] Volume applies to audio player immediately
- [ ] Volume applies on app startup

**Step 3: Commit (if any fixes needed)**

```bash
git add -A
git commit -m "fix: address settings integration issues"
```

---

## Summary

This plan creates a complete settings system with:
1. Settings module with auto-save
2. Dedicated settings screen with j/k/h/l navigation
3. Volume control that applies immediately
4. Loop mode that integrates with playback
5. Persistence between app restarts
