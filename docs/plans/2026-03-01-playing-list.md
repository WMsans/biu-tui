# Playing List Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a persistent playing list feature that allows users to queue songs, manage the queue, and control playback with loop modes.

**Architecture:** Separate PlayingListManager handles queue logic and persistence. LibraryScreen adds 4th tab for playing list display and operations. App integrates manager and handles playback completion with new loop modes.

**Tech Stack:** Rust, serde for JSON persistence, Ratatui for TUI, parking_lot::Mutex for thread-safe state

---

## Task 1: Update LoopMode Enum

**Files:**
- Modify: `src/storage/settings.rs:6-10`

**Step 1: Update LoopMode enum**

Remove `LoopFolder` variant and add `LoopList` variant:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    LoopOne,
    NoLoop,
    LoopList,
}
```

**Step 2: Update next() method**

Update the `next()` method to cycle through the new modes:

```rust
pub fn next(self) -> Self {
    match self {
        LoopMode::LoopOne => LoopMode::NoLoop,
        LoopMode::NoLoop => LoopMode::LoopList,
        LoopMode::LoopList => LoopMode::LoopOne,
    }
}
```

**Step 3: Update prev() method**

Update the `prev()` method:

```rust
pub fn prev(self) -> Self {
    match self {
        LoopMode::LoopOne => LoopMode::LoopList,
        LoopMode::NoLoop => LoopMode::LoopOne,
        LoopMode::LoopList => LoopMode::NoLoop,
    }
}
```

**Step 4: Update display_name() method**

Update display names:

```rust
pub fn display_name(&self) -> &str {
    match self {
        LoopMode::LoopOne => "Loop One",
        LoopMode::NoLoop => "No Loop",
        LoopMode::LoopList => "Loop List",
    }
}
```

**Step 5: Run tests to verify**

Run: `cargo test`
Expected: All tests pass (existing tests may fail due to enum change)

**Step 6: Fix any failing tests**

If tests reference `LoopFolder`, update them to use new modes.

**Step 7: Commit**

```bash
git add src/storage/settings.rs
git commit -m "feat: replace LoopFolder with LoopList mode"
```

---

## Task 2: Create PlaylistItem Struct

**Files:**
- Create: `src/playing_list/mod.rs`

**Step 1: Create module directory and file**

Run: `mkdir -p src/playing_list && touch src/playing_list/mod.rs`

**Step 2: Add PlaylistItem struct**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub bvid: String,
    pub cid: u64,
    pub title: String,
    pub artist: String,
    pub duration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayingListData {
    items: Vec<PlaylistItem>,
    current_index: Option<usize>,
}
```

**Step 3: Commit**

```bash
git add src/playing_list/
git commit -m "feat: add PlaylistItem struct"
```

---

## Task 3: Implement PlayingListManager Core

**Files:**
- Modify: `src/playing_list/mod.rs`

**Step 1: Add PlayingListManager struct**

```rust
use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct PlayingListManager {
    items: Vec<PlaylistItem>,
    current_index: Option<usize>,
    storage_path: PathBuf,
}
```

**Step 2: Implement constructor with load**

```rust
impl PlayingListManager {
    pub fn new() -> Result<Self> {
        let storage_path = crate::storage::Settings::settings_dir()?
            .join("playing_list.json");
        
        let mut manager = Self {
            items: Vec::new(),
            current_index: None,
            storage_path,
        };
        
        manager.load()?;
        Ok(manager)
    }
    
    fn load(&mut self) -> Result<()> {
        if !self.storage_path.exists() {
            return Ok(());
        }
        
        let content = std::fs::read_to_string(&self.storage_path)
            .context("Failed to read playing list file")?;
        
        if content.trim().is_empty() {
            return Ok(());
        }
        
        let data: PlayingListData = serde_json::from_str(&content)
            .context("Failed to parse playing list")?;
        
        self.items = data.items;
        self.current_index = data.current_index;
        
        if let Some(idx) = self.current_index {
            if idx >= self.items.len() {
                self.current_index = None;
            }
        }
        
        Ok(())
    }
}
```

**Step 3: Implement save method**

```rust
    fn save(&self) -> Result<()> {
        let data = PlayingListData {
            items: self.items.clone(),
            current_index: self.current_index,
        };
        
        let content = serde_json::to_string_pretty(&data)
            .context("Failed to serialize playing list")?;
        
        let temp_path = self.storage_path.with_extension("json.tmp");
        std::fs::write(&temp_path, content)
            .context("Failed to write playing list")?;
        
        std::fs::rename(&temp_path, &self.storage_path)
            .context("Failed to save playing list")?;
        
        Ok(())
    }
```

**Step 4: Implement add methods**

```rust
    pub fn add(&mut self, item: PlaylistItem) {
        self.items.push(item);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
        let _ = self.save();
    }
    
    pub fn add_all(&mut self, items: Vec<PlaylistItem>) {
        let was_empty = self.items.is_empty();
        self.items.extend(items);
        if was_empty && !self.items.is_empty() {
            self.current_index = Some(0);
        }
        let _ = self.save();
    }
```

**Step 5: Implement remove method**

```rust
    pub fn remove(&mut self, index: usize) -> Option<PlaylistItem> {
        if index >= self.items.len() {
            return None;
        }
        
        let item = self.items.remove(index);
        
        if let Some(current) = self.current_index {
            if current == index {
                if self.items.is_empty() {
                    self.current_index = None;
                } else if current >= self.items.len() {
                    self.current_index = Some(self.items.len() - 1);
                }
            } else if current > index {
                self.current_index = Some(current - 1);
            }
        }
        
        let _ = self.save();
        Some(item)
    }
```

**Step 6: Implement jump_to and navigation methods**

```rust
    pub fn jump_to(&mut self, index: usize) {
        if index < self.items.len() {
            self.current_index = Some(index);
            let _ = self.save();
        }
    }
    
    pub fn current(&self) -> Option<&PlaylistItem> {
        self.current_index.and_then(|idx| self.items.get(idx))
    }
    
    pub fn next(&mut self) -> Option<&PlaylistItem> {
        let current = self.current_index?;
        
        if current + 1 < self.items.len() {
            self.current_index = Some(current + 1);
        } else {
            self.current_index = Some(0);
        }
        
        let _ = self.save();
        self.current()
    }
    
    pub fn items(&self) -> &[PlaylistItem] {
        &self.items
    }
    
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }
    
    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        let _ = self.save();
    }
```

**Step 7: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 8: Commit**

```bash
git add src/playing_list/mod.rs
git commit -m "feat: implement PlayingListManager with persistence"
```

---

## Task 4: Add Playing List to Storage Module

**Files:**
- Create: `src/storage/playing_list.rs`
- Modify: `src/storage/mod.rs`

**Step 1: Create storage wrapper**

Create `src/storage/playing_list.rs`:

```rust
pub use crate::playing_list::{PlaylistItem, PlayingListManager};
```

**Step 2: Export from storage module**

Add to `src/storage/mod.rs`:

```rust
pub mod playing_list;

pub use playing_list::{PlaylistItem, PlayingListManager};
```

**Step 3: Commit**

```bash
git add src/storage/
git commit -m "feat: export PlayingListManager from storage module"
```

---

## Task 5: Add PlayingListManager to App

**Files:**
- Modify: `src/app.rs`

**Step 1: Add field to App struct**

Add after `settings` field (line 35):

```rust
pub struct App {
    // ... existing fields ...
    settings: Settings,
    playing_list: Arc<Mutex<PlayingListManager>>,
    previous_library: Option<LibraryScreen>,
    // ...
}
```

**Step 2: Initialize in App::new()**

Add after settings initialization (around line 50):

```rust
let settings = Settings::load().unwrap_or_default();
let playing_list = Arc::new(Mutex::new(
    PlayingListManager::new().unwrap_or_else(|e| {
        eprintln!("Failed to load playing list: {}", e);
        PlayingListManager::new_empty().unwrap()
    })
));
```

**Step 3: Add to return statement**

Update the return in `App::new()`:

```rust
Ok(Self {
    terminal,
    running: true,
    screen,
    client,
    player,
    downloader,
    config,
    last_qr_poll,
    settings,
    playing_list,
    previous_library,
    previous_player_state,
})
```

**Step 4: Add helper method to PlayingListManager**

Add to `src/playing_list/mod.rs`:

```rust
impl PlayingListManager {
    pub fn new_empty() -> Result<Self> {
        let storage_path = crate::storage::Settings::settings_dir()?
            .join("playing_list.json");
        
        Ok(Self {
            items: Vec::new(),
            current_index: None,
            storage_path,
        })
    }
}
```

**Step 5: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 6: Commit**

```bash
git add src/app.rs src/playing_list/mod.rs
git commit -m "feat: add PlayingListManager to App"
```

---

## Task 6: Add PlayingList Tab to LibraryScreen

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add PlayingList variant to LibraryTab enum**

Add after `History` (line 18):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Favorites,
    WatchLater,
    History,
    PlayingList,
}
```

**Step 2: Update tab cycling in handle_key**

In `src/app.rs`, update the Tab key handler (around line 155):

```rust
KeyCode::Tab => {
    library.current_tab = match library.current_tab {
        LibraryTab::Favorites => LibraryTab::WatchLater,
        LibraryTab::WatchLater => LibraryTab::History,
        LibraryTab::History => LibraryTab::PlayingList,
        LibraryTab::PlayingList => LibraryTab::Favorites,
    };
}
```

**Step 3: Update render method to show PlayingList tab**

In `src/screens/library.rs`, update the titles vector (around line 155):

```rust
let titles: Vec<&str> = vec!["Favorites", "Watch Later", "History", "Playing List"];
```

**Step 4: Update current_list_len method**

Add case for PlayingList:

```rust
fn current_list_len(&self) -> usize {
    match self.current_tab {
        LibraryTab::Favorites => {
            match &self.nav_level {
                NavigationLevel::Folders => self.folders.len(),
                NavigationLevel::Videos { .. } => self.resources.len(),
                NavigationLevel::Episodes { .. } => self.episodes.len(),
            }
        }
        LibraryTab::WatchLater => self.watch_later.len(),
        LibraryTab::History => self.history.len(),
        LibraryTab::PlayingList => 0, // Will update later
    }
}
```

**Step 5: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 6: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: add PlayingList tab to LibraryScreen"
```

---

## Task 7: Render Playing List Tab

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add playing_list parameter to render**

Update the render method signature (line 142):

```rust
pub fn render(
    &mut self,
    f: &mut Frame,
    area: Rect,
    player: Option<&AudioPlayer>,
    playing_list: Arc<Mutex<PlayingListManager>>,
)
```

**Step 2: Update items vector to handle PlayingList tab**

Update the items construction (around line 176):

```rust
let items: Vec<ListItem> = match self.current_tab {
    LibraryTab::Favorites => {
        // ... existing code ...
    }
    LibraryTab::WatchLater => {
        // ... existing code ...
    }
    LibraryTab::History => {
        // ... existing code ...
    }
    LibraryTab::PlayingList => {
        let list = playing_list.lock();
        list.items()
            .iter()
            .enumerate()
            .map(|(idx, item)| {
                let current_marker = if Some(idx) == list.current_index() {
                    "♫"
                } else {
                    " "
                };
                ListItem::new(format!(
                    "{} {} - {}  {}",
                    current_marker,
                    item.title,
                    item.artist,
                    format_duration(item.duration)
                ))
            })
            .collect()
    }
};
```

**Step 3: Update current_list_len for PlayingList**

```rust
LibraryTab::PlayingList => {
    let list = playing_list.lock();
    list.items().len()
}
```

**Step 4: Update App to pass playing_list to render**

In `src/app.rs`, update the render call (around line 113):

```rust
Screen::Library(library) => {
    let mut lib = library.clone();
    lib.render(f, area, self.player.as_ref(), self.playing_list.clone());
}
```

**Step 5: Update render signature in method calls**

Find all other places where `library.render()` is called and update them similarly.

**Step 6: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 7: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: render playing list items in PlayingList tab"
```

---

## Task 8: Implement Add Song Operation

**Files:**
- Modify: `src/screens/library.rs`
- Modify: `src/app.rs`

**Step 1: Add add_to_playing_list method to LibraryScreen**

Add new method after `play_selected`:

```rust
pub fn add_to_playing_list(
    &self,
    playing_list: Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>,
) -> anyhow::Result<()> {
    let (bvid, title, artist, duration) = match self.current_tab {
        LibraryTab::Favorites => {
            if let Some(idx) = self.list_state.selected() {
                if let NavigationLevel::Videos { .. } = &self.nav_level {
                    self.resources.get(idx).map(|r| {
                        (r.bvid.clone(), r.title.clone(), r.upper.name.clone(), r.duration)
                    })
                } else {
                    None
                }
            } else {
                None
            }
        }
        LibraryTab::WatchLater => {
            if let Some(idx) = self.list_state.selected() {
                self.watch_later.get(idx).map(|w| {
                    (w.bvid.clone(), w.title.clone(), 
                     w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                     w.duration)
                })
            } else {
                None
            }
        }
        LibraryTab::History => {
            if let Some(idx) = self.list_state.selected() {
                self.history.get(idx).and_then(|h| {
                    h.bvid.as_ref().map(|bvid| {
                        (bvid.clone(), h.title.clone(),
                         h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                         h.duration)
                    })
                })
            } else {
                None
            }
        }
        LibraryTab::PlayingList => None,
    };
    
    if let Some((bvid, title, artist, duration)) = (bvid, title, artist, duration) {
        let cid = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            let video_info = rt.block_on(client.get_video_info(&bvid))?;
            video_info.cid
        };
        
        let item = PlaylistItem {
            bvid,
            cid,
            title,
            artist,
            duration,
        };
        
        playing_list.lock().add(item);
    }
    
    Ok(())
}
```

**Step 2: Add key handler in App**

In `src/app.rs`, add handler for 'a' key in Library screen (around line 170):

```rust
KeyCode::Char('a') => {
    if let Err(e) = library.add_to_playing_list(
        self.playing_list.clone(),
        self.client.clone(),
    ) {
        eprintln!("Failed to add to playing list: {}", e);
    }
}
```

**Step 3: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 4: Test manually**

Run: `cargo run`
Navigate to a folder with songs, select one, press 'a', switch to Playing List tab to verify.

**Step 5: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: add 'a' key to add song to playing list"
```

---

## Task 9: Implement Add All Songs Operation

**Files:**
- Modify: `src/screens/library.rs`
- Modify: `src/app.rs`

**Step 1: Add add_all_to_playing_list method**

Add to LibraryScreen:

```rust
pub fn add_all_to_playing_list(
    &self,
    playing_list: Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>,
) -> anyhow::Result<()> {
    let items: Vec<PlaylistItem> = match self.current_tab {
        LibraryTab::Favorites => {
            if let NavigationLevel::Videos { .. } = &self.nav_level {
                let mut items = Vec::new();
                for resource in &self.resources {
                    let cid = {
                        let client = client.lock();
                        let rt = tokio::runtime::Runtime::new()?;
                        let video_info = rt.block_on(client.get_video_info(&resource.bvid))?;
                        video_info.cid
                    };
                    
                    items.push(PlaylistItem {
                        bvid: resource.bvid.clone(),
                        cid,
                        title: resource.title.clone(),
                        artist: resource.upper.name.clone(),
                        duration: resource.duration,
                    });
                }
                Some(items)
            } else {
                None
            }
        }
        _ => None,
    };
    
    if let Some(items) = items {
        if !items.is_empty() {
            playing_list.lock().add_all(items);
        }
    }
    
    Ok(())
}
```

**Step 2: Add key handler for Shift+A**

In `src/app.rs`, add handler for 'A' (around line 180):

```rust
KeyCode::Char('A') => {
    if let Err(e) = library.add_all_to_playing_list(
        self.playing_list.clone(),
        self.client.clone(),
    ) {
        eprintln!("Failed to add all to playing list: {}", e);
    }
}
```

**Step 3: Update help text**

Update help text in library.rs render method (around line 296):

```rust
let help = Paragraph::new(
    "[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [a] Add to list  [A] Add all  [Tab] Switch"
)
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: add 'A' key to add all songs to playing list"
```

---

## Task 10: Implement Jump to Song

**Files:**
- Modify: `src/screens/library.rs`
- Modify: `src/app.rs`

**Step 1: Add handle_jump_to_song method**

Add to LibraryScreen:

```rust
pub fn handle_jump_to_song(
    &mut self,
    playing_list: Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
) -> anyhow::Result<()> {
    if self.current_tab != LibraryTab::PlayingList {
        return Ok(());
    }
    
    let selected_idx = match self.list_state.selected() {
        Some(idx) => idx,
        None => return Ok(()),
    };
    
    let item = {
        let mut list = playing_list.lock();
        list.jump_to(selected_idx);
        match list.current().cloned() {
            Some(item) => item,
            None => return Ok(()),
        }
    };
    
    let audio_stream = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
    };
    
    if player.is_none() {
        *player = Some(AudioPlayer::new()?);
    }
    
    if let Some(p) = player {
        p.play(&audio_stream.url)?;
        self.now_playing = Some((item.title, item.artist));
    }
    
    Ok(())
}
```

**Step 2: Update handle_enter for PlayingList tab**

In `handle_enter` method, add case for PlayingList:

```rust
pub fn handle_enter(
    &mut self,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
    playing_list: Arc<Mutex<PlayingListManager>>,
) -> anyhow::Result<()> {
    match self.current_tab {
        LibraryTab::PlayingList => {
            self.handle_jump_to_song(playing_list, client, player)?;
        }
        LibraryTab::Favorites => {
            // ... existing code ...
        }
        LibraryTab::WatchLater | LibraryTab::History => {
            // ... existing code ...
        }
    }
    Ok(())
}
```

**Step 3: Update handle_enter call in App**

In `src/app.rs`, update the Enter key handler (around line 164):

```rust
KeyCode::Enter => {
    if let Err(e) = library.handle_enter(
        self.client.clone(),
        &mut self.player,
        self.playing_list.clone(),
    ) {
        eprintln!("Failed to handle enter: {}", e);
    }
    self.apply_volume();
}
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: implement jump to song in PlayingList tab"
```

---

## Task 11: Implement Remove Song

**Files:**
- Modify: `src/screens/library.rs`
- Modify: `src/app.rs`

**Step 1: Add handle_remove_song method**

Add to LibraryScreen:

```rust
pub fn handle_remove_song(
    &mut self,
    playing_list: Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
) -> anyhow::Result<()> {
    if self.current_tab != LibraryTab::PlayingList {
        return Ok(());
    }
    
    let selected_idx = match self.list_state.selected() {
        Some(idx) => idx,
        None => return Ok(()),
    };
    
    let current_idx = playing_list.lock().current_index();
    let is_current = current_idx == Some(selected_idx);
    
    playing_list.lock().remove(selected_idx);
    
    if is_current {
        let next_item = playing_list.lock().current().cloned();
        
        if let Some(item) = next_item {
            let audio_stream = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
            };
            
            if let Some(p) = player {
                p.play(&audio_stream.url)?;
                self.now_playing = Some((item.title, item.artist));
            }
        } else {
            if let Some(p) = player {
                p.stop();
            }
            self.now_playing = None;
        }
    }
    
    let list_len = playing_list.lock().items().len();
    if selected_idx >= list_len && list_len > 0 {
        self.list_state.select(Some(list_len - 1));
    }
    
    Ok(())
}
```

**Step 2: Add key handler for 'd' key**

In `src/app.rs`, add handler (around line 175):

```rust
KeyCode::Char('d') => {
    if let Err(e) = library.handle_remove_song(
        self.playing_list.clone(),
        self.client.clone(),
        &mut self.player,
    ) {
        eprintln!("Failed to remove song: {}", e);
    }
}
```

**Step 3: Update help text for PlayingList tab**

In render method, show context-sensitive help:

```rust
let help_text = match self.current_tab {
    LibraryTab::PlayingList => {
        "[j/k] Navigate  [Enter] Jump  [d] Remove  [Tab] Switch"
    }
    _ => {
        "[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [a] Add  [A] Add all  [Tab] Switch"
    }
};
let help = Paragraph::new(help_text).block(Block::default().borders(Borders::TOP));
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: implement remove song with 'd' key"
```

---

## Task 12: Update Playback Completion Logic

**Files:**
- Modify: `src/app.rs`

**Step 1: Update poll_playback_completion method**

Update the method to use PlayingListManager (around line 284):

```rust
fn poll_playback_completion(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        let current_state = self.player.as_ref().map(|p| p.state());
        
        let was_playing = self.previous_player_state == Some(PlayerState::Playing);
        let now_stopped = current_state == Some(PlayerState::Stopped);
        
        if was_playing && now_stopped {
            match self.settings.loop_mode {
                LoopMode::LoopOne => {
                    self.replay_current()?;
                }
                LoopMode::NoLoop => {
                    if let Some(item) = self.playing_list.lock().next().cloned() {
                        self.play_playlist_item(&item)?;
                    }
                }
                LoopMode::LoopList => {
                    let _ = self.playing_list.lock().next();
                    if let Some(item) = self.playing_list.lock().current().cloned() {
                        self.play_playlist_item(&item)?;
                    }
                }
            }
        }
        
        self.previous_player_state = current_state;
    }
    Ok(())
}
```

**Step 2: Add play_playlist_item helper method**

Add new method to App:

```rust
fn play_playlist_item(&mut self, item: &PlaylistItem) -> Result<()> {
    let audio_stream = {
        let client = self.client.lock();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
    };
    
    if self.player.is_none() {
        self.player = Some(AudioPlayer::new()?);
    }
    
    if let Some(p) = &mut self.player {
        p.play(&audio_stream.url)?;
        if let Screen::Library(library) = &mut self.screen {
            library.now_playing = Some((item.title.clone(), item.artist.clone()));
        }
    }
    
    self.apply_volume();
    Ok(())
}
```

**Step 3: Update replay_current to use playing list**

Update the replay_current method:

```rust
fn replay_current(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        if let Some(item) = self.playing_list.lock().current().cloned() {
            self.play_playlist_item(&item)?;
        } else if let Err(e) = library.handle_enter(
            self.client.clone(),
            &mut self.player,
            self.playing_list.clone(),
        ) {
            eprintln!("Failed to replay: {}", e);
        }
        self.apply_volume();
    }
    Ok(())
}
```

**Step 4: Run cargo check**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: update playback completion with LoopList mode"
```

---

## Task 13: Write Unit Tests

**Files:**
- Modify: `src/playing_list/mod.rs`
- Modify: `src/storage/settings.rs`

**Step 1: Add tests for PlayingListManager**

Add at end of `src/playing_list/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    
    fn create_test_manager() -> PlayingListManager {
        let dir = tempdir().unwrap();
        let path = dir.path().join("playing_list.json");
        
        PlayingListManager {
            items: Vec::new(),
            current_index: None,
            storage_path: path,
        }
    }
    
    fn create_test_item(id: usize) -> PlaylistItem {
        PlaylistItem {
            bvid: format!("BV{}", id),
            cid: id as u64,
            title: format!("Song {}", id),
            artist: format!("Artist {}", id),
            duration: 180,
        }
    }
    
    #[test]
    fn test_add_item() {
        let mut manager = create_test_manager();
        let item = create_test_item(1);
        
        manager.add(item.clone());
        
        assert_eq!(manager.items().len(), 1);
        assert_eq!(manager.current_index(), Some(0));
        assert_eq!(manager.current(), Some(&item));
    }
    
    #[test]
    fn test_add_multiple_items() {
        let mut manager = create_test_manager();
        
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        manager.add(create_test_item(3));
        
        assert_eq!(manager.items().len(), 3);
        assert_eq!(manager.current_index(), Some(0));
    }
    
    #[test]
    fn test_remove_item() {
        let mut manager = create_test_manager();
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        
        let removed = manager.remove(1);
        
        assert!(removed.is_some());
        assert_eq!(manager.items().len(), 1);
    }
    
    #[test]
    fn test_remove_current_item_advances_to_next() {
        let mut manager = create_test_manager();
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        manager.jump_to(0);
        
        manager.remove(0);
        
        assert_eq!(manager.current_index(), Some(0));
        assert_eq!(manager.current().unwrap().bvid, "BV2");
    }
    
    #[test]
    fn test_jump_to_song() {
        let mut manager = create_test_manager();
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        manager.add(create_test_item(3));
        
        manager.jump_to(2);
        
        assert_eq!(manager.current_index(), Some(2));
        assert_eq!(manager.current().unwrap().bvid, "BV3");
    }
    
    #[test]
    fn test_next_wraps_around() {
        let mut manager = create_test_manager();
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        manager.jump_to(1);
        
        manager.next();
        
        assert_eq!(manager.current_index(), Some(0));
    }
    
    #[test]
    fn test_persistence_save_and_load() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("playing_list.json");
        
        let mut manager = PlayingListManager {
            items: Vec::new(),
            current_index: None,
            storage_path: path.clone(),
        };
        
        manager.add(create_test_item(1));
        manager.add(create_test_item(2));
        manager.jump_to(1);
        
        let loaded_manager = PlayingListManager {
            items: Vec::new(),
            current_index: None,
            storage_path: path,
        };
        
        let mut loaded_manager = loaded_manager;
        loaded_manager.load().unwrap();
        
        assert_eq!(loaded_manager.items().len(), 2);
        assert_eq!(loaded_manager.current_index(), Some(1));
    }
    
    #[test]
    fn test_empty_list_operations() {
        let manager = create_test_manager();
        
        assert_eq!(manager.items().len(), 0);
        assert_eq!(manager.current_index(), None);
        assert_eq!(manager.current(), None);
    }
}
```

**Step 2: Add tests for LoopMode**

Add at end of `src/storage/settings.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_loop_mode_next_sequence() {
        assert_eq!(LoopMode::LoopOne.next(), LoopMode::NoLoop);
        assert_eq!(LoopMode::NoLoop.next(), LoopMode::LoopList);
        assert_eq!(LoopMode::LoopList.next(), LoopMode::LoopOne);
    }
    
    #[test]
    fn test_loop_mode_prev_sequence() {
        assert_eq!(LoopMode::LoopOne.prev(), LoopMode::LoopList);
        assert_eq!(LoopMode::NoLoop.prev(), LoopMode::LoopOne);
        assert_eq!(LoopMode::LoopList.prev(), LoopMode::NoLoop);
    }
    
    #[test]
    fn test_loop_folder_removed() {
        let modes = vec![LoopMode::LoopOne, LoopMode::NoLoop, LoopMode::LoopList];
        assert!(!modes.iter().any(|m| matches!(m, LoopMode::LoopFolder)));
    }
}
```

**Step 3: Add tempfile dependency**

Add to `Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Commit**

```bash
git add src/playing_list/mod.rs src/storage/settings.rs Cargo.toml
git commit -m "test: add unit tests for PlayingListManager and LoopMode"
```

---

## Task 14: Final Integration and Testing

**Step 1: Run full test suite**

Run: `cargo test --all`
Expected: All tests pass

**Step 2: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Format code**

Run: `cargo fmt`

**Step 4: Build release**

Run: `cargo build --release`
Expected: Build succeeds

**Step 5: Manual testing**

Run: `cargo run --release`

Test checklist:
1. Navigate to Favorites tab
2. Enter a folder with multiple songs
3. Press 'a' to add single song → verify appears in Playing List tab
4. Press 'A' to add all songs → verify all appear in Playing List tab
5. Go to Playing List tab
6. Press Enter on a song → verify it plays
7. Press 'd' on a non-playing song → verify it's removed
8. Press 'd' on currently playing song → verify it skips to next
9. Change loop mode to LoopList → verify playback loops
10. Change loop mode to NoLoop → verify playback stops at end
11. Restart app → verify playing list is restored

**Step 6: Fix any issues found**

If any issues found during manual testing, create fix commits.

**Step 7: Final commit**

```bash
git add .
git commit -m "chore: final integration and testing"
```

---

## Summary

This implementation plan creates a fully functional playing list feature with:

- ✅ Persistent queue storage
- ✅ 4th tab in Library screen
- ✅ Add single/all songs operations
- ✅ Jump to song and remove operations
- ✅ LoopList mode for queue looping
- ✅ Graceful error handling
- ✅ Comprehensive unit tests

**Total estimated tasks:** 14  
**Estimated implementation time:** 4-6 hours  
**Files created:** 2  
**Files modified:** 5  
**New lines of code:** ~400
