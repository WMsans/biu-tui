# Multi-Playlist Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add named playlist management (create/delete/rename/reorder), rename "Playing List" to "Playing Now", add Playlists tab, implement conditional breadcrumbs and unified "add to" prompt.

**Architecture:** New `src/playlists/mod.rs` module for `PlaylistManager` (one JSON file per playlist + index file). Playlists become the 5th `LibraryTab` variant. A modal `AddPromptState` overlays content for choosing playlist targets. Breadcrumb rendering becomes conditional per tab.

**Tech Stack:** Rust, Ratatui, serde_json, parking_lot::Mutex, anyhow, tempfile (test-only)

---

## File Structure Map

| File | Action | Responsibility |
|---|---|---|
| `src/playlists/mod.rs` | Create | PlaylistManager: CRUD, reorder, persistence |
| `src/lib.rs` | Modify | Add `pub mod playlists;` |
| `src/storage/mod.rs` | Modify | Re-export PlaylistManager |
| `src/screens/library.rs` | Modify | Playlists tab, AddPromptState, conditional breadcrumb, key handling |
| `src/app.rs` | Modify | Wire playlist_manager, rename PlayingList→PlayingNow, unified add prompt routing |

---

### Task 1: Create PlaylistManager module

**Files:**
- Create: `src/playlists/mod.rs`
- Modify: `src/lib.rs:6`
- Modify: `src/storage/mod.rs:1`

- [ ] **Step 1: Write PlaylistManager with tests**

Write `src/playlists/mod.rs`:

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<PlaylistItem>,
}

pub struct PlaylistManager {
    base_dir: PathBuf,
}

impl PlaylistManager {
    pub fn new() -> Result<Self> {
        let base_dir = crate::storage::Settings::settings_dir()?.join("playlists");
        std::fs::create_dir_all(&base_dir).context("Failed to create playlists directory")?;

        let manager = Self { base_dir };
        if !manager.index_path().exists() {
            manager.rebuild_index()?;
        }
        Ok(manager)
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join(".index.json")
    }

    fn playlist_path(&self, name: &str) -> PathBuf {
        let sanitized: String = name
            .chars()
            .map(|c| if c == '/' || c == '\\' || c == ':' || c == '*' || c == '?' || c == '"' || c == '<' || c == '>' || c == '|' { '_' } else { c })
            .collect();
        self.base_dir.join(format!("{}.json", sanitized))
    }

    fn load_index(&self) -> Result<Vec<String>> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path).context("Failed to read index file")?;
        let names: Vec<String> =
            serde_json::from_str(&content).context("Failed to parse index file")?;
        Ok(names)
    }

    fn save_index(&self, names: &[String]) -> Result<()> {
        let path = self.index_path();
        let content =
            serde_json::to_string_pretty(names).context("Failed to serialize index")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, content).context("Failed to write index")?;
        std::fs::rename(&temp, &path).context("Failed to save index")?;
        Ok(())
    }

    fn rebuild_index(&self) -> Result<()> {
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".json") && !name.starts_with('.') {
                    let without_ext = &name[..name.len() - 5];
                    names.push(without_ext.to_string());
                }
            }
        }
        names.sort();
        self.save_index(&names)
    }

    pub fn list_names(&self) -> Result<Vec<String>> {
        let names = self.load_index()?;
        let mut valid = Vec::new();
        let mut needs_fix = false;
        for name in &names {
            if self.playlist_path(name).exists() {
                valid.push(name.clone());
            } else {
                needs_fix = true;
            }
        }
        if needs_fix {
            self.save_index(&valid)?;
        }
        Ok(valid)
    }

    pub fn load(&self, name: &str) -> Result<Playlist> {
        let path = self.playlist_path(name);
        if !path.exists() {
            return Ok(Playlist {
                name: name.to_string(),
                items: Vec::new(),
            });
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read playlist file: {}", name))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse playlist: {}", name))
    }

    fn save(&self, playlist: &Playlist) -> Result<()> {
        let path = self.playlist_path(&playlist.name);
        let content =
            serde_json::to_string_pretty(playlist).context("Failed to serialize playlist")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, content).context("Failed to write playlist")?;
        std::fs::rename(&temp, &path).context("Failed to save playlist")?;
        Ok(())
    }

    pub fn create(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Playlist name cannot be empty");
        }
        if name.len() > 64 {
            anyhow::bail!("Playlist name too long (max 64 characters)");
        }
        let path = self.playlist_path(name);
        if path.exists() {
            anyhow::bail!("Playlist '{}' already exists", name);
        }
        let playlist = Playlist {
            name: name.to_string(),
            items: Vec::new(),
        };
        self.save(&playlist)?;
        let mut names = self.load_index()?;
        names.push(name.to_string());
        names.sort();
        self.save_index(&names)?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.playlist_path(name);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to delete playlist file: {}", name))?;
        }
        let mut names = self.load_index()?;
        names.retain(|n| n != name);
        self.save_index(&names)?;
        Ok(())
    }

    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            anyhow::bail!("Playlist name cannot be empty");
        }
        if new_name.len() > 64 {
            anyhow::bail!("Playlist name too long (max 64 characters)");
        }
        if self.playlist_path(new_name).exists() {
            anyhow::bail!("Playlist '{}' already exists", new_name);
        }
        let old_path = self.playlist_path(old_name);
        if !old_path.exists() {
            anyhow::bail!("Playlist '{}' does not exist", old_name);
        }

        let mut playlist = self.load(old_name)?;
        playlist.name = new_name.to_string();
        let new_path = self.playlist_path(new_name);
        let content =
            serde_json::to_string_pretty(&playlist).context("Failed to serialize playlist")?;
        std::fs::write(&new_path, content).context("Failed to write renamed playlist")?;

        if old_path != new_path {
            std::fs::remove_file(&old_path).ok();
        }

        let mut names = self.load_index()?;
        if let Some(pos) = names.iter().position(|n| n == old_name) {
            names[pos] = new_name.to_string();
        }
        names.sort();
        self.save_index(&names)?;
        Ok(())
    }

    pub fn add_item(&self, playlist_name: &str, item: PlaylistItem) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        playlist.items.push(item);
        self.save(&playlist)?;
        Ok(())
    }

    pub fn remove_item(&self, playlist_name: &str, index: usize) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        if index < playlist.items.len() {
            playlist.items.remove(index);
            self.save(&playlist)?;
        }
        Ok(())
    }

    pub fn reorder(&self, playlist_name: &str, from: usize, to: usize) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        let len = playlist.items.len();
        if from < len && to < len && from != to {
            let item = playlist.items.remove(from);
            playlist.items.insert(to, item);
            self.save(&playlist)?;
        }
        Ok(())
    }

    pub fn get_items(&self, playlist_name: &str) -> Result<Vec<PlaylistItem>> {
        let playlist = self.load(playlist_name)?;
        Ok(playlist.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_item(id: usize) -> PlaylistItem {
        PlaylistItem {
            bvid: format!("BV{}", id),
            cid: id as u64,
            title: format!("Song {}", id),
            artist: format!("Artist {}", id),
            duration: 180,
        }
    }

    struct TestEnv {
        manager: PlaylistManager,
        _dir: tempfile::TempDir,
    }

    fn setup() -> TestEnv {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join("playlists");
        std::fs::create_dir_all(&base_dir).unwrap();

        // Override settings_dir — we can't, so test the PlaylistManager directly
        // by providing a custom base_dir through a test constructor
        // We test via the public API that touches filesystem paths we control
        let manager = PlaylistManager { base_dir };
        TestEnv {
            manager,
            _dir: dir,
        }
    }

    #[test]
    fn test_create_and_list_playlists() {
        let env = setup();
        env.manager.create("Workout").unwrap();
        env.manager.create("Study").unwrap();

        let names = env.manager.list_names().unwrap();
        assert_eq!(names, vec!["Study", "Workout"]);
    }

    #[test]
    fn test_create_duplicate_rejected() {
        let env = setup();
        env.manager.create("Test").unwrap();
        assert!(env.manager.create("Test").is_err());
    }

    #[test]
    fn test_create_empty_name_rejected() {
        let env = setup();
        assert!(env.manager.create("").is_err());
        assert!(env.manager.create("   ").is_err());
    }

    #[test]
    fn test_create_name_too_long_rejected() {
        let env = setup();
        let long_name = "a".repeat(65);
        assert!(env.manager.create(&long_name).is_err());
    }

    #[test]
    fn test_delete_playlist() {
        let env = setup();
        env.manager.create("TempPlaylist").unwrap();
        env.manager.delete("TempPlaylist").unwrap();
        assert!(env.manager.list_names().unwrap().is_empty());
    }

    #[test]
    fn test_rename_playlist() {
        let env = setup();
        env.manager.create("OldName").unwrap();
        env.manager.rename("OldName", "NewName").unwrap();

        let names = env.manager.list_names().unwrap();
        assert_eq!(names, vec!["NewName"]);
        assert!(!env.manager.playlist_path("OldName").exists());
        assert!(env.manager.playlist_path("NewName").exists());
    }

    #[test]
    fn test_rename_to_existing_rejected() {
        let env = setup();
        env.manager.create("First").unwrap();
        env.manager.create("Second").unwrap();
        assert!(env.manager.rename("First", "Second").is_err());
    }

    #[test]
    fn test_add_and_get_items() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();

        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Song 1");
    }

    #[test]
    fn test_remove_item() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();
        env.manager.add_item("Songs", create_test_item(3)).unwrap();

        env.manager.remove_item("Songs", 1).unwrap();

        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Song 1");
        assert_eq!(items[1].title, "Song 3");
    }

    #[test]
    fn test_reorder_items() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();
        env.manager.add_item("Songs", create_test_item(3)).unwrap();

        env.manager.reorder("Songs", 0, 2).unwrap();

        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items[0].title, "Song 2");
        assert_eq!(items[1].title, "Song 3");
        assert_eq!(items[2].title, "Song 1");
    }

    #[test]
    fn test_reorder_noop_same_index() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();

        env.manager.reorder("Songs", 0, 0).unwrap();

        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_reorder_out_of_bounds_noop() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();

        env.manager.reorder("Songs", 0, 99).unwrap();

        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_persistence_roundtrip() {
        let env = setup();
        env.manager.create("Persist").unwrap();
        env.manager.add_item("Persist", create_test_item(1)).unwrap();
        env.manager.add_item("Persist", create_test_item(2)).unwrap();

        // Reload by creating a new manager pointing to same dir
        let manager2 = PlaylistManager {
            base_dir: env.manager.base_dir.clone(),
        };
        let items = manager2.get_items("Persist").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(manager2.list_names().unwrap(), vec!["Persist"]);
    }

    #[test]
    fn test_empty_playlist() {
        let env = setup();
        env.manager.create("Empty").unwrap();
        let items = env.manager.get_items("Empty").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_index_rebuild_from_files() {
        let env = setup();
        env.manager.create("Alpha").unwrap();
        env.manager.create("Beta").unwrap();

        // Corrupt index by removing it, then recreate manager
        std::fs::remove_file(env.manager.index_path()).unwrap();
        let manager2 = PlaylistManager {
            base_dir: env.manager.base_dir.clone(),
        };

        let names = manager2.list_names().unwrap();
        assert_eq!(names, vec!["Alpha", "Beta"]);
    }
}
```

- [ ] **Step 2: Run tests to verify PlaylistManager**

```bash
cargo test playlists::
```
Expected: all tests pass.

- [ ] **Step 3: Add `pub mod playlists;` to `src/lib.rs`**

In `src/lib.rs`, add after line 6 (`pub mod playing_list;`):
```rust
pub mod playlists;
```

- [ ] **Step 4: Add re-export in `src/storage/mod.rs`**

In `src/storage/mod.rs`, add after line 3 (`pub mod playing_list;`):
```rust
pub mod playlists;
```

And add after the `pub use playing_list::{PlayingListManager, PlaylistItem};` line:
```rust
pub use playlists::PlaylistManager;
```

- [ ] **Step 5: Commit**

```bash
git add src/playlists/mod.rs src/lib.rs src/storage/mod.rs
git commit -m "feat: add PlaylistManager with full CRUD and reorder"
```

---

### Task 2: Wire PlaylistManager into App

**Files:**
- Modify: `src/app.rs:17,40,82-85,118`

- [ ] **Step 1: Add imports and field**

In `src/app.rs`, add import after line 17 (`use crate::playing_list::{PlayingListManager, PlaylistItem};`):
```rust
use crate::playlists::PlaylistManager;
```

In `App` struct (line 40), add field after `playing_list`:
```rust
    playlist_manager: Arc<Mutex<PlaylistManager>>,
```

- [ ] **Step 2: Initialize in init_app_state**

Replace lines 82-85:
```rust
        let playing_list = Arc::new(Mutex::new(PlayingListManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to load playing list: {}", e);
            PlayingListManager::new_empty().unwrap()
        })));
```

with:
```rust
        let playing_list = Arc::new(Mutex::new(PlayingListManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to load playing list: {}", e);
            PlayingListManager::new_empty().unwrap()
        })));
        let playlist_manager = Arc::new(Mutex::new(PlaylistManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to load playlists: {}", e);
            PlaylistManager::new().unwrap()
        })));
```

- [ ] **Step 3: Add playlist_manager to Ok(Self{...}) constructor**

After the `playing_list` field (around line 118), add:
```rust
            playlist_manager,
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check
```

- [ ] **Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: wire PlaylistManager into App"
```

---

### Task 3: Rename PlayingList to PlayingNow and add Playlists tab variant

**Files:**
- Modify: `src/screens/library.rs:16-22`
- Modify: `src/screens/library.rs` — all references
- Modify: `src/app.rs` — all references

- [ ] **Step 1: Update LibraryTab enum**

In `src/screens/library.rs`, replace lines 16-22:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Favorites,
    WatchLater,
    History,
    PlayingList,
}
```

with:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Favorites,
    WatchLater,
    History,
    PlayingNow,
    Playlists,
}
```

- [ ] **Step 2: Replace all `PlayingList` with `PlayingNow` in library.rs**

```bash
cd /home/jeremy/Development/Webapp/biu-tui && rg -n "PlayingList" src/screens/library.rs
```

These are the lines that refer to `LibraryTab::PlayingList`. Update them all to `LibraryTab::PlayingNow`. Here are the exact replacements:

**Line 324-344:** `LibraryTab::PlayingList => {` → `LibraryTab::PlayingNow => {`

**Line 433:** `LibraryTab::PlayingList => {` → `LibraryTab::PlayingNow => {`

**Line 510:** `LibraryTab::PlayingList => playing_list.lock().items().len(),` → `LibraryTab::PlayingNow => ...`

**Line 521:** `self.current_tab != LibraryTab::PlayingList` → `self.current_tab != LibraryTab::PlayingNow`

**Line 591:** `LibraryTab::PlayingList => {` → `LibraryTab::PlayingNow => {`

**Line 755:** `LibraryTab::PlayingList => None,` → `LibraryTab::PlayingNow => None,`

**Line 859:** `LibraryTab::PlayingList => {` → `LibraryTab::PlayingNow => {`

**Line 978:** `LibraryTab::PlayingList => {` → `LibraryTab::PlayingNow => {`

**Line 1105-1106:** `if self.current_tab != LibraryTab::PlayingList` → `if self.current_tab != LibraryTab::PlayingNow`

- [ ] **Step 3: Add Playlists arm to all match blocks in library.rs**

For every match on `self.current_tab`, add a `LibraryTab::Playlists` arm. Since many functions only operate on specific tabs, the default `_` arm is often fine, but explicit arms are needed for:

1. **`current_list_len_with_playlist()`** (line 498-511): Add:
```rust
LibraryTab::Playlists => self.playlist_items.len(),
```

2. **`handle_enter()`** (line 564-596): Add:
```rust
LibraryTab::Playlists => {
    self.handle_playlists_enter(playing_list, client, player, playback_speed)?;
}
```

3. **`go_back()`** (line 995-1016): Add at start of match:
```rust
if self.current_tab == LibraryTab::Playlists {
    match &self.playlist_nav_level {
        PlaylistNavLevel::PlaylistContents { .. } => {
            self.playlist_nav_level = PlaylistNavLevel::PlaylistList;
            self.playlist_items.clear();
            self.list_state.select(Some(0));
        }
        PlaylistNavLevel::PlaylistList => {}
    }
    return;
}
```

4. **`handle_remove_song()`** (line 1099-1148): Gate already handles by checking `!== LibraryTab::PlayingNow`, so it's fine. But add to the tab check at line 1106:
```rust
if self.current_tab != LibraryTab::PlayingNow && self.current_tab != LibraryTab::Playlists {
    return Ok(());
}
```

5. **`add_to_playing_list()`** (line 859): The `PlayingNow` arm already returns. Add an explicit arm:
```rust
LibraryTab::Playlists => {
    self.status_message = Some("Use 'a' from other tabs to add to playlists".to_string());
    return Ok(());
}
```

6. **`add_all_to_playing_list()`** (line 978): Same:
```rust
LibraryTab::Playlists => {
    self.status_message = Some("Use 'A' from other tabs to add all to playlists".to_string());
    return Ok(());
}
```

- [ ] **Step 4: Replace all `PlayingList` with `PlayingNow` in app.rs**

```bash
cd /home/jeremy/Development/Webapp/biu-tui && rg -n "PlayingList" src/app.rs
```

Replace each `LibraryTab::PlayingList` with `LibraryTab::PlayingNow`. Key lines:
- Line 277: tab cycling
- Line 278: tab cycling (needs `Playlists` added too)
- Line 529-541: search handling

Also update tab cycling at lines 274-279:
```rust
library.current_tab = match library.current_tab {
    LibraryTab::Favorites => LibraryTab::WatchLater,
    LibraryTab::WatchLater => LibraryTab::History,
    LibraryTab::History => LibraryTab::PlayingNow,
    LibraryTab::PlayingNow => LibraryTab::Playlists,
    LibraryTab::Playlists => LibraryTab::Favorites,
};
```

Update the title at line 229 from `"Playing List"` to `"Playing Now"`.

Update search handling at line 529: `LibraryTab::PlayingNow => {`

Update the search match block to handle Playlists:
```rust
LibraryTab::Playlists => {
    // Local playlist search: filter playlist_items
    library.playlist_items = library
        .playlist_items
        .iter()
        .filter(|item| item.matches(query))
        .cloned()
        .collect();
}
```

Actually, for search on Playlists tab, we need original state save/restore too. Let me handle this in the search task later. For now, skip search on Playlists tab (`_ => {}`).

- [ ] **Step 5: Run cargo check**

```bash
cargo check 2>&1
```

Expected: may have errors about missing `PlaylistNavLevel`, `playlist_items`, `AddPromptState`, `handle_playlists_enter`, `toggle_add_prompt` — these come in later tasks. That's fine, this is incremental.

- [ ] **Step 6: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "refactor: rename PlayingList to PlayingNow, add Playlists tab variant"
```

---

### Task 4: Add playlist state fields and new types to LibraryScreen

**Files:**
- Modify: `src/screens/library.rs` (struct definition and `new()`)

- [ ] **Step 1: Define new types**

Add after `NavigationLevel` enum (after line 37):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistNavLevel {
    PlaylistList,
    PlaylistContents { name: String },
}

#[derive(Debug, Clone)]
pub struct AddPromptState {
    pub is_active: bool,
    pub destinations: Vec<String>,
    pub selected: usize,
    pub source_title: String,
    pub mode: AddPromptMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddPromptMode {
    Single,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistEditMode {
    Creating,
    Renaming { old_name: String },
}
```

- [ ] **Step 2: Add fields to LibraryScreen struct**

Add after `original_history` (line 71):

```rust
    pub playlist_nav_level: PlaylistNavLevel,
    pub playlist_items: Vec<PlaylistItem>,
    pub playlist_names: Vec<String>,
    pub add_prompt_state: Option<AddPromptState>,
    pub playlist_edit_mode: Option<PlaylistEditMode>,
    // Cached data for the add-all prompt (Favorites/History items)
    pub add_all_candidates: Vec<PlaylistItem>,
```

- [ ] **Step 3: Initialize in `new()`**

In the `Self { }` block (after `original_history` line 106), add:

```rust
            playlist_nav_level: PlaylistNavLevel::PlaylistList,
            playlist_items: Vec::new(),
            playlist_names: Vec::new(),
            add_prompt_state: None,
            playlist_edit_mode: None,
            add_all_candidates: Vec::new(),
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add playlist state fields and AddPromptState to LibraryScreen"
```

---

### Task 5: Update render for Playlists tab and conditional breadcrumb

**Files:**
- Modify: `src/screens/library.rs:204-446` (render function)

With all the render changes, the `render` function signature needs to also accept `playlist_manager`. Let me adapt it.

- [ ] **Step 1: Update render signature and layout**

Update the render signature (line 204-210) to accept playlist_manager:

```rust
    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        player: Option<&AudioPlayer>,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
```

Replace the constraints and title line (lines 211-235):

```rust
        // Determine if breadcrumb row should show
        let show_breadcrumb = matches!(
            self.current_tab,
            LibraryTab::Favorites | LibraryTab::Playlists
        );

        let mut constraints = vec![Constraint::Length(3)];
        if show_breadcrumb {
            constraints.push(Constraint::Length(1));
        }
        if self.search_state.is_some() {
            constraints.push(Constraint::Length(3));
        }
        constraints.extend(vec![
            Constraint::Min(10),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
        ]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let titles: Vec<&str> =
            vec!["Favorites", "Watch Later", "History", "Playing Now", "Playlists"];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.current_tab as usize)
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Cyan));
        f.render_widget(tabs, chunks[0]);
```

Replace the breadcrumb block (lines 237-251):

```rust
        let mut chunk_offset = 1;

        if show_breadcrumb {
            let breadcrumb_text = match self.current_tab {
                LibraryTab::Favorites => match &self.nav_level {
                    NavigationLevel::Folders => "Favorites".to_string(),
                    NavigationLevel::Videos { folder_title, .. } => {
                        format!("Favorites > {}", folder_title)
                    }
                    NavigationLevel::Episodes {
                        folder_id_title,
                        video_title,
                        ..
                    } => {
                        format!("Favorites > {} > {}", folder_id_title, video_title)
                    }
                },
                LibraryTab::Playlists => match &self.playlist_nav_level {
                    PlaylistNavLevel::PlaylistList => "Playlists".to_string(),
                    PlaylistNavLevel::PlaylistContents { name } => {
                        format!("Playlists > {}", name)
                    }
                },
                _ => String::new(),
            };
            let breadcrumb =
                Paragraph::new(breadcrumb_text).style(Style::default().fg(Color::Yellow));
            f.render_widget(breadcrumb, chunks[chunk_offset]);
            chunk_offset += 1;
        }
```

- [ ] **Step 2: Add Playlists tab rendering to the List items block**

After line 344 (`}` closing the `PlayingNow` arm), and before `};` (line 345), add:

```rust
            LibraryTab::Playlists => match &self.playlist_nav_level {
                PlaylistNavLevel::PlaylistList => {
                    if self.playlist_names.is_empty() {
                        vec![ListItem::new("No playlists yet. Press 'n' to create one.")]
                    } else {
                        // Refresh playlist items count from manager for display
                        let pm = playlist_manager.lock();
                        self.playlist_names
                            .iter()
                            .map(|name| {
                                let count = pm.get_items(name)
                                    .map(|items| items.len())
                                    .unwrap_or(0);
                                ListItem::new(format!("{} ({})", name, count))
                            })
                            .collect()
                    }
                }
                PlaylistNavLevel::PlaylistContents { .. } => {
                    self.playlist_items
                        .iter()
                        .enumerate()
                        .map(|(_idx, item)| {
                            ListItem::new(format!(
                                "{} - {}  {}",
                                item.title,
                                item.artist,
                                format_duration(item.duration)
                            ))
                        })
                        .collect()
                }
            },
```

- [ ] **Step 3: Fix chunk_offset for the list area**

The list rendering currently uses `chunks[chunk_offset]`. After the breadcrumb changes, `chunk_offset` starts at 1 or 2. Let me verify the index usage is correct by replacing the section that renders search bar (lines 253-259):

Replace:
```rust
        let mut chunk_offset = 2;

        if let Some(ref search_state) = self.search_state {
```

with search bar rendering that accounts for the new chunk_offset:
```rust
        if let Some(ref search_state) = self.search_state {
            SearchBar::new(&search_state.query, search_state.cursor_position)
                .render(f, chunks[chunk_offset]);
            chunk_offset += 1;
        }
```

Remove the old `let mut chunk_offset = 2;` line — we now set it in Step 1.

- [ ] **Step 4: Update the help bar for Playlists tab**

In the help text match at lines 432-441, add:

```rust
                LibraryTab::Playlists => match &self.playlist_nav_level {
                    PlaylistNavLevel::PlaylistList => {
                        "[j/k] Navigate  [Enter] Open  [n] New  [d] Delete  [r] Rename  [Tab] Switch"
                    }
                    PlaylistNavLevel::PlaylistContents { .. } => {
                        "[j/k] Navigate  [Enter] Play  [d] Remove  [Ctrl+↑/↓] Reorder  [Esc] Back  [Tab] Switch"
                    }
                },
```

- [ ] **Step 5: Load playlist names on render refresh for Playlists tab**

At the top of the render function (before layout), add:
```rust
        if self.current_tab == LibraryTab::Playlists {
            if let Ok(names) = playlist_manager.lock().list_names() {
                self.playlist_names = names;
            }
        }
```

- [ ] **Step 6: Add render of AddPromptState overlay**

After the help bar (after line 445, before the closing `}` of render), add:

```rust
        // Render add prompt overlay if active
        if let Some(ref prompt) = self.add_prompt_state {
            if prompt.is_active {
                let prompt_title = match prompt.mode {
                    AddPromptMode::Single => format!("Add '{}' to:", prompt.source_title),
                    AddPromptMode::All => {
                        format!("Add {} items to:", self.add_all_candidates.len())
                    }
                };
                let mut prompt_lines: Vec<String> =
                    vec![prompt_title, String::new()];
                for (i, dest) in prompt.destinations.iter().enumerate() {
                    let marker = if i == prompt.selected { "> " } else { "  " };
                    prompt_lines.push(format!("{}{}", marker, dest));
                }
                prompt_lines.push(String::new());
                prompt_lines.push("[j/k] Select  [Enter] Confirm  [Esc] Cancel".to_string());

                let prompt_height = prompt_lines.len() as u16 + 2;
                let prompt_width = 50u16;
                let prompt_x = area.width.saturating_sub(prompt_width) / 2;
                let prompt_y = area.height.saturating_sub(prompt_height) / 2;
                let prompt_area = Rect::new(prompt_x, prompt_y, prompt_width, prompt_height);

                let prompt_text = prompt_lines.join("\n");
                let prompt_widget = Paragraph::new(prompt_text)
                    .block(Block::default().borders(Borders::ALL).title("Add To"))
                    .style(Style::default().fg(Color::Yellow));
                f.render_widget(prompt_widget, prompt_area);
            }
        }
```

- [ ] **Step 7: Update app.rs render call**

In `src/app.rs`, find the render line (line 155):
```rust
lib.render(f, area, self.player.as_ref(), self.playing_list.clone());
```

Replace with:
```rust
lib.render(
    f,
    area,
    self.player.as_ref(),
    self.playing_list.clone(),
    self.playlist_manager.clone(),
);
```

- [ ] **Step 8: Run cargo check**

```bash
cargo check 2>&1
```

- [ ] **Step 9: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: render Playlists tab, conditional breadcrumb, and add prompt overlay"
```

---

### Task 6: Implement Playlists tab key handling

**Files:**
- Modify: `src/screens/library.rs` (new methods)

- [ ] **Step 1: Add handle_playlists_enter method**

Add after `format_time` function (end of file before `}`):

```rust
    fn handle_playlists_enter(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playback_speed: f32,
    ) -> anyhow::Result<()> {
        match &self.playlist_nav_level.clone() {
            PlaylistNavLevel::PlaylistList => {
                let idx = match self.list_state.selected() {
                    Some(i) if i < self.playlist_names.len() => i,
                    _ => return Ok(()),
                };
                let name = self.playlist_names[idx].clone();
                let pm = playlist_manager.lock();
                self.playlist_items = pm.get_items(&name)?;
                drop(pm);
                self.playlist_nav_level = PlaylistNavLevel::PlaylistContents { name };
                self.list_state.select(Some(0));
            }
            PlaylistNavLevel::PlaylistContents { name } => {
                let name = name.clone();
                let idx = match self.list_state.selected() {
                    Some(i) if i < self.playlist_items.len() => i,
                    _ => {
                        self.status_message = Some("No tracks in playlist".to_string());
                        return Ok(());
                    }
                };
                // Load all playlist items into Playing Now
                let items = self.playlist_items.clone();
                {
                    let mut pl = playing_list.lock();
                    pl.clear();
                    pl.add_all(items.clone());
                    pl.jump_to(idx);
                }
                // Start playback at selected track
                let item = &self.playlist_items[idx];
                let audio_stream = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
                };
                if player.is_none() {
                    *player = Some(AudioPlayer::new()?);
                }
                if let Some(p) = player {
                    p.play(&audio_stream.url, playback_speed)?;
                    self.now_playing = Some((item.title.clone(), item.artist.clone()));
                }
                self.status_message = Some(format!("Loaded '{}' into Playing Now", name));
            }
        }
        Ok(())
    }
```

- [ ] **Step 2: Add handle_playlists_create method**

```rust
    pub fn handle_playlists_create(
        &mut self,
        _playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
        if self.search_state.is_some() {
            return;
        }
        self.playlist_edit_mode = Some(PlaylistEditMode::Creating);
        self.search_state = Some(crate::screens::SearchState::new());
        self.status_message = Some("Type playlist name and press Enter to create, Esc to cancel".to_string());
    }

    pub fn handle_playlists_delete(
        &mut self,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_names.len() => i,
            _ => return,
        };
        let name = self.playlist_names[idx].clone();
        match playlist_manager.lock().delete(&name) {
            Ok(_) => {
                self.playlist_names.remove(idx);
                if idx >= self.playlist_names.len() && !self.playlist_names.is_empty() {
                    self.list_state.select(Some(self.playlist_names.len() - 1));
                } else if self.playlist_names.is_empty() {
                    self.list_state.select(None);
                }
                self.status_message = Some(format!("Deleted '{}'", name));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to delete: {}", e));
            }
        }
    }

    pub fn handle_playlists_rename(
        &mut self,
        _playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_names.len() => i,
            _ => return,
        };
        let name = self.playlist_names[idx].clone();
        if self.search_state.is_some() {
            return;
        }
        self.playlist_edit_mode = Some(PlaylistEditMode::Renaming {
            old_name: name.clone(),
        });
        self.search_state = Some(crate::screens::SearchState::new());
        self.status_message = Some(format!("Renaming '{}': type new name and press Enter", name));
    }

    pub fn handle_playlists_remove_item(
        &mut self,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_items.len() => i,
            _ => return,
        };
        let name = match &self.playlist_nav_level {
            PlaylistNavLevel::PlaylistContents { name } => name.clone(),
            _ => return,
        };
        match playlist_manager.lock().remove_item(&name, idx) {
            Ok(_) => {
                self.playlist_items.remove(idx);
                if idx >= self.playlist_items.len() && !self.playlist_items.is_empty() {
                    self.list_state.select(Some(self.playlist_items.len() - 1));
                } else if self.playlist_items.is_empty() {
                    self.list_state.select(None);
                }
                self.status_message = Some("Removed from playlist".to_string());
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to remove: {}", e));
            }
        }
    }

    pub fn handle_playlists_reorder(
        &mut self,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        direction_up: bool,
    ) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_items.len() => i,
            _ => return,
        };
        let name = match &self.playlist_nav_level {
            PlaylistNavLevel::PlaylistContents { name } => name.clone(),
            _ => return,
        };

        let to = if direction_up {
            if idx == 0 { return; }
            idx - 1
        } else {
            if idx + 1 >= self.playlist_items.len() { return; }
            idx + 1
        };

        match playlist_manager.lock().reorder(&name, idx, to) {
            Ok(_) => {
                let item = self.playlist_items.remove(idx);
                self.playlist_items.insert(to, item);
                self.list_state.select(Some(to));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to reorder: {}", e));
            }
        }
    }
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check 2>&1
```

- [ ] **Step 4: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add Playlists tab key handlers (enter, create, delete, rename, remove, reorder)"
```

---

### Task 7: Implement unified add prompt (activate, navigate, confirm)

**Files:**
- Modify: `src/screens/library.rs`

- [ ] **Step 1: Add toggle_add_prompt method**

Add after the existing playlist handling methods:

```rust
    pub fn toggle_add_prompt(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        mode: AddPromptMode,
    ) {
        if self.add_prompt_state.is_some() {
            self.add_prompt_state = None;
            return;
        }

        let idx = match self.list_state.selected() {
            Some(i) => i,
            None => {
                self.status_message = Some("No item selected".to_string());
                return;
            }
        };

        let source_title = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Folders => {
                    self.status_message =
                        Some("Navigate into a folder first".to_string());
                    return;
                }
                NavigationLevel::Videos { .. } => {
                    self.resources.get(idx).map(|r| r.title.clone())
                        .unwrap_or_else(|| "Unknown".to_string())
                }
                NavigationLevel::Episodes { .. } => {
                    self.episodes.get(idx).map(|e| e.part.clone())
                        .unwrap_or_else(|| "Unknown".to_string())
                }
            },
            LibraryTab::WatchLater => {
                self.watch_later.get(idx).map(|w| w.title.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            }
            LibraryTab::History => {
                self.history.get(idx).map(|h| h.title.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            }
            _ => {
                self.status_message = Some("Nothing to add here".to_string());
                return;
            }
        };

        // Build destination list
        let mut destinations = vec!["Playing Now".to_string()];
        if let Ok(names) = playlist_manager.lock().list_names() {
            destinations.extend(names);
        }

        if destinations.len() <= 1 {
            // No playlists exist, add directly to Playing Now
            self.add_to_playing_list(playing_list, None);
            return;
        }

        self.add_prompt_state = Some(AddPromptState {
            is_active: true,
            destinations,
            selected: 0,
            source_title,
            mode,
        });
    }
```

- [ ] **Step 2: Update add_to_playing_list to accept optional client (for direct mode)**

Update the signature of `add_to_playing_list` (line 784-786) to accept `Option<Arc<Mutex<BilibiliClient>>>`:

Actually, let me keep the existing method and add a `resolve_add_target` method that dispatches:

```rust
    pub fn resolve_add_target(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<()> {
        let prompt = match &self.add_prompt_state {
            Some(p) if p.is_active => p.clone(),
            _ => return Ok(()),
        };

        let selected_dest = prompt.destinations[prompt.selected].clone();

        if selected_dest == "Playing Now" {
            // Use existing add logic
            if prompt.mode == AddPromptMode::All {
                return self.add_all_to_playing_list(playing_list, client);
            }
            // Clear prompt so add_to_playing_list doesn't recurse
            self.add_prompt_state = None;
            let result = self.add_to_playing_list(playing_list, client);
            self.add_prompt_state = None;
            self.status_message = Some("Added to Playing Now".to_string());
            return result;
        }

        // Add to a named playlist
        if prompt.mode == AddPromptMode::All {
            // Build items from add_all_candidates
            let count = self.add_all_candidates.len();
            for item in &self.add_all_candidates {
                playlist_manager
                    .lock()
                    .add_item(&selected_dest, item.clone())?;
            }
            self.add_prompt_state = None;
            self.status_message =
                Some(format!("Added {} items to '{}'", count, selected_dest));
            return Ok(());
        }

        // Single item: use the same metadata extraction as add_to_playing_list
        let item = self.extract_selected_playlist_item(client.clone())?;
        playlist_manager.lock().add_item(&selected_dest, item)?;
        self.add_prompt_state = None;
        self.status_message =
            Some(format!("Added to '{}'", selected_dest));
        Ok(())
    }
```

And extract the metadata logic into a helper:

```rust
    fn extract_selected_playlist_item(
        &self,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<PlaylistItem> {
        let idx = self.list_state.selected()
            .ok_or_else(|| anyhow::anyhow!("No item selected"))?;

        // For Episodes, use existing data (no API call needed)
        if let LibraryTab::Favorites = self.current_tab {
            if let NavigationLevel::Episodes { bvid, .. } = &self.nav_level {
                if let Some(episode) = self.episodes.get(idx) {
                    let artist = self
                        .current_video_info
                        .as_ref()
                        .map(|v| v.owner.name.clone())
                        .unwrap_or_default();
                    return Ok(PlaylistItem {
                        bvid: bvid.clone(),
                        cid: episode.cid,
                        title: episode.part.clone(),
                        artist,
                        duration: episode.duration,
                    });
                }
            }
        }

        let (bvid, title, artist, duration) = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Videos { .. } => self.resources.get(idx).map(|r| {
                    (r.bvid.clone(), r.title.clone(), r.upper.name.clone(), r.duration)
                }).ok_or_else(|| anyhow::anyhow!("No resource at index"))?,
                _ => anyhow::bail!("Navigate into a folder first"),
            },
            LibraryTab::WatchLater => self.watch_later.get(idx).map(|w| {
                (w.bvid.clone(), w.title.clone(),
                 w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                 w.duration)
            }).ok_or_else(|| anyhow::anyhow!("No item at index"))?,
            LibraryTab::History => {
                let h = self.history.get(idx)
                    .ok_or_else(|| anyhow::anyhow!("No item at index"))?;
                let bvid = h.bvid.clone()
                    .ok_or_else(|| anyhow::anyhow!("History item has no bvid"))?;
                (bvid, h.title.clone(),
                 h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                 h.duration)
            }
            _ => anyhow::bail!("Cannot add from this tab"),
        };

        let cid = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            let video_info = rt.block_on(client.get_video_info(&bvid))?;
            video_info.cid
        };

        Ok(PlaylistItem {
            bvid,
            cid,
            title,
            artist,
            duration,
        })
    }
```

- [ ] **Step 3: Add handle_add_prompt_key method**

```rust
    pub fn handle_add_prompt_key(
        &mut self,
        key: ratatui::crossterm::event::KeyCode,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<bool> {
        let prompt = match &mut self.add_prompt_state {
            Some(p) if p.is_active => p,
            _ => return Ok(false),
        };

        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if prompt.selected + 1 < prompt.destinations.len() {
                    prompt.selected += 1;
                }
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if prompt.selected > 0 {
                    prompt.selected -= 1;
                }
                Ok(true)
            }
            KeyCode::Enter => {
                self.resolve_add_target(playing_list, playlist_manager, client)?;
                Ok(true)
            }
            KeyCode::Esc => {
                self.add_prompt_state = None;
                Ok(true)
            }
            _ => Ok(true),
        }
    }
```

You'll need the import at the top of library.rs:
```rust
use ratatui::crossterm::event::KeyCode;
```

Actually, `KeyCode` is already imported from crossterm via app.rs. In library.rs, use `crossterm::event::KeyCode` or just `KeyCode` as used in app.rs.

- [ ] **Step 4: Run cargo check**

```bash
cargo check 2>&1
```

- [ ] **Step 5: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement unified add prompt with playlist target selection"
```

---

### Task 8: Wire everything in app.rs key handling

**Files:**
- Modify: `src/app.rs` (handle_key and related)

- [ ] **Step 1: Update key handling for 'a' and 'A'**

In `src/app.rs`, find the KeyCode::Char('a') block (around line 304). Replace:

```rust
                    KeyCode::Char('a') => {
                        if let Err(e) = library
                            .add_to_playing_list(self.playing_list.clone(), self.client.clone())
                        {
                            library.status_message =
                                Some(format!("Failed to add to playing list: {}", e));
                        }
                    }
```

with:
```rust
                    KeyCode::Char('a') => {
                        library.toggle_add_prompt(
                            self.playing_list.clone(),
                            self.playlist_manager.clone(),
                            AddPromptMode::Single,
                        );
                    }
```

Replace the `KeyCode::Char('A')` block similarly:

```rust
                    KeyCode::Char('A') => {
                        library.toggle_add_prompt(
                            self.playing_list.clone(),
                            self.playlist_manager.clone(),
                            AddPromptMode::All,
                        );
                    }
```

- [ ] **Step 2: Add add-prompt key capture**

Before the main key match block, add a check for add prompt:

```rust
                    // Handle add prompt keys first (modal)
                    if library.add_prompt_state.is_some() {
                        let handled = library.handle_add_prompt_key(
                            code,
                            self.playing_list.clone(),
                            self.playlist_manager.clone(),
                            self.client.clone(),
                        );
                        if let Err(e) = handled {
                            library.status_message =
                                Some(format!("Add failed: {}", e));
                        }
                        return Ok(());
                    }
```

- [ ] **Step 3: Add Playlists tab key handling**

After the `KeyCode::Char('d')` block (around line 320), add:

```rust
                    KeyCode::Char('n') => {
                        if library.current_tab == LibraryTab::Playlists
                            && library.playlist_nav_level == PlaylistNavLevel::PlaylistList
                        {
                            library.handle_playlists_create(self.playlist_manager.clone());
                        }
                    }
                    KeyCode::Char('r') => {
                        if library.current_tab == LibraryTab::Playlists
                            && library.playlist_nav_level == PlaylistNavLevel::PlaylistList
                        {
                            library.handle_playlists_rename(self.playlist_manager.clone());
                        }
                    }
                    KeyCode::Char('u') => {
                        // Ctrl+Up for reorder up — we can't detect Ctrl+Up easily
                        // Use 'u' as reorder up and 'U' as reorder down on playlist contents
                        if library.current_tab == LibraryTab::Playlists
                            && matches!(library.playlist_nav_level, PlaylistNavLevel::PlaylistContents { .. })
                        {
                            library
                                .handle_playlists_reorder(self.playlist_manager.clone(), true);
                        }
                    }
                    KeyCode::Char('U') => {
                        if library.current_tab == LibraryTab::Playlists
                            && matches!(library.playlist_nav_level, PlaylistNavLevel::PlaylistContents { .. })
                        {
                            library
                                .handle_playlists_reorder(self.playlist_manager.clone(), false);
                        }
                    }
```

Update the `KeyCode::Char('d')` block to also handle Playlists tab:

Replace:
```rust
                    KeyCode::Char('d') => {
                        let old_now_playing = library.now_playing.clone();
                        if let Err(e) = library.handle_remove_song(
                            self.playing_list.clone(),
                            self.client.clone(),
                            &mut self.player,
                            self.settings.playback_speed,
                        ) {
                            eprintln!("Failed to remove song: {}", e);
                        }
                        let new_now_playing = library.now_playing.clone();
                        self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                    }
```

with:
```rust
                    KeyCode::Char('d') => {
                        if library.current_tab == LibraryTab::Playlists {
                            if matches!(
                                library.playlist_nav_level,
                                PlaylistNavLevel::PlaylistContents { .. }
                            ) {
                                library
                                    .handle_playlists_remove_item(self.playlist_manager.clone());
                            } else if matches!(
                                library.playlist_nav_level,
                                PlaylistNavLevel::PlaylistList
                            ) {
                                library
                                    .handle_playlists_delete(self.playlist_manager.clone());
                            }
                        } else {
                            let old_now_playing = library.now_playing.clone();
                            if let Err(e) = library.handle_remove_song(
                                self.playing_list.clone(),
                                self.client.clone(),
                                &mut self.player,
                                self.settings.playback_speed,
                            ) {
                                eprintln!("Failed to remove song: {}", e);
                            }
                            let new_now_playing = library.now_playing.clone();
                            self.notify_mpris_if_playback_changed(
                                &old_now_playing,
                                &new_now_playing,
                            );
                        }
                    }
```

- [ ] **Step 4: Update Esc/Backspace for playlist edit cancel and go_back**

In `src/app.rs`, replace the Esc/Backspace handler:
```rust
                    KeyCode::Esc | KeyCode::Backspace => library.go_back(),
```

with:
```rust
                    KeyCode::Esc | KeyCode::Backspace => {
                        // Cancel playlist edit mode first
                        if library.playlist_edit_mode.is_some() {
                            library.playlist_edit_mode = None;
                            library.search_state = None;
                            library.status_message = None;
                        } else {
                            library.go_back();
                        }
                    }
```

In `library.rs` `go_back()`, add at the beginning:
```rust
    pub fn go_back(&mut self) {
        if self.current_tab == LibraryTab::Playlists {
            match &self.playlist_nav_level {
                PlaylistNavLevel::PlaylistContents { .. } => {
                    self.playlist_nav_level = PlaylistNavLevel::PlaylistList;
                    self.playlist_items.clear();
                    self.list_state.select(Some(0));
                }
                PlaylistNavLevel::PlaylistList => {}
            }
            return;
        }
        match &self.nav_level {
            // ... existing code
```

- [ ] **Step 5: Update search/create/rename flow integration**

When the user types a playlist name in the search bar (create/rename mode), we need Enter to finalize. Update the `Enter` key handler to detect if we're in "create playlist" mode.

In `src/app.rs`, update the Enter key handler (around line 288-301) to check for playlist create mode:

Replace:
```rust
                    KeyCode::Enter => {
                        let old_now_playing = library.now_playing.clone();
                        if let Err(e) = library.handle_enter(
                            self.client.clone(),
                            &mut self.player,
                            self.playing_list.clone(),
                            self.settings.playback_speed,
                        ) {
                            eprintln!("Failed to handle enter: {}", e);
                        }
                        let new_now_playing = library.now_playing.clone();
                        self.apply_volume();
                        self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                    }
```

with:
```rust
                    KeyCode::Enter => {
                        // Check if we're in playlist create/rename mode (search bar used as name input)
                        if library.current_tab == LibraryTab::Playlists
                            && library.playlist_edit_mode.is_some()
                        {
                            let edit_mode = library.playlist_edit_mode.take().unwrap();
                            let query = library.search_state.as_ref()
                                .map(|s| s.query.clone())
                                .unwrap_or_default();
                            library.search_state = None;

                            match edit_mode {
                                PlaylistEditMode::Creating => {
                                    match self.playlist_manager.lock().create(&query) {
                                        Ok(_) => {
                                            library.status_message =
                                                Some(format!("Created '{}'", query));
                                        }
                                        Err(e) => {
                                            library.status_message =
                                                Some(format!("Create failed: {}", e));
                                        }
                                    }
                                }
                                PlaylistEditMode::Renaming { old_name } => {
                                    match self.playlist_manager.lock().rename(&old_name, &query) {
                                        Ok(_) => {
                                            library.status_message =
                                                Some(format!("Renamed to '{}'", query));
                                        }
                                        Err(e) => {
                                            library.status_message =
                                                Some(format!("Rename failed: {}", e));
                                        }
                                    }
                                }
                            }
                            return Ok(());
                        }

                        let old_now_playing = library.now_playing.clone();
                        if let Err(e) = library.handle_enter(
                            self.client.clone(),
                            &mut self.player,
                            self.playing_list.clone(),
                            self.settings.playback_speed,
                        ) {
                            eprintln!("Failed to handle enter: {}", e);
                        }
                        let new_now_playing = library.now_playing.clone();
                        self.apply_volume();
                        self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                    }
```

- [ ] **Step 6: Add imports to app.rs**

Add at top of app.rs (after existing imports from screens):
```rust
use crate::screens::library::{AddPromptMode, PlaylistNavLevel};
```

- [ ] **Step 7: Update handle_enter in library.rs for Playlists**

Update the `handle_enter` method: it already has `LibraryTab::Playlists => { self.handle_playlists_enter(...) }` from Task 3. That's correct.

- [ ] **Step 8: Run cargo check and fix any compilation errors**

```bash
cargo check 2>&1
```

- [ ] **Step 9: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: wire playlist key handling and add prompt in app event loop"
```

---

### Task 9: Final cleanup, fixes, and verification

**Files:**
- Modify: `src/screens/library.rs`
- Modify: `src/app.rs`

- [ ] **Step 1: Fix `add_all_to_playing_list` to handle Playlists exclusion**

The `add_all_to_playing_list` method still tries to match on every tab. The Playlists arm is already handled (returns early). But we also need to build `add_all_candidates` when `toggle_add_prompt` is called with `AddPromptMode::All`. Add this to `toggle_add_prompt`:

After building the source_title for Single mode, add for All mode:

```rust
        if mode == AddPromptMode::All {
            // Build candidate list for "add all"
            let candidates: Vec<PlaylistItem> = match self.current_tab {
                LibraryTab::Favorites => match &self.nav_level {
                    NavigationLevel::Videos { .. } => {
                        let client_lock = playlist_manager.clone();
                        // We need the real client... 
                        // For now, build items with cid=0 and fetch later
                        // Actually, we can't easily build PlaylistItems without the client
                        // Store the tab/level state and fetch in resolve_add_target
                        Vec::new() // Defer to resolve time
                    }
                    NavigationLevel::Episodes { bvid, .. } => {
                        let artist = self
                            .current_video_info
                            .as_ref()
                            .map(|v| v.owner.name.clone())
                            .unwrap_or_default();
                        self.episodes
                            .iter()
                            .map(|ep| PlaylistItem {
                                bvid: bvid.clone(),
                                cid: ep.cid,
                                title: ep.part.clone(),
                                artist: artist.clone(),
                                duration: ep.duration,
                            })
                            .collect()
                    }
                    _ => Vec::new(),
                },
                LibraryTab::WatchLater => {
                    self.watch_later
                        .iter()
                        .map(|w| PlaylistItem {
                            bvid: w.bvid.clone(),
                            cid: 0,
                            title: w.title.clone(),
                            artist: w.owner.as_ref()
                                .map(|o| o.name.clone()).unwrap_or_default(),
                            duration: w.duration,
                        })
                        .collect()
                }
                LibraryTab::History => {
                    self.history
                        .iter()
                        .filter_map(|h| h.bvid.as_ref().map(|bvid| PlaylistItem {
                            bvid: bvid.clone(),
                            cid: 0,
                            title: h.title.clone(),
                            artist: h.owner.as_ref()
                                .map(|o| o.name.clone()).unwrap_or_default(),
                            duration: h.duration,
                        }))
                        .collect()
                }
                _ => Vec::new(),
            };
            self.add_all_candidates = candidates;
            if candidates.is_empty() {
                self.status_message = Some("No items to add".to_string());
                return;
            }
        }
```

And update `resolve_add_target` for All mode to use the candidates:

```rust
        if prompt.mode == AddPromptMode::All {
            if selected_dest == "Playing Now" {
                return self.add_all_to_playing_list(playing_list, client);
            }
            let count = self.add_all_candidates.len();
            let rt = tokio::runtime::Runtime::new()?;
            for mut item in self.add_all_candidates.clone() {
                if item.cid == 0 {
                    let cid = {
                        let client = client.lock();
                        rt.block_on(client.get_video_info(&item.bvid))?.cid
                    };
                    item.cid = cid;
                }
                playlist_manager.lock().add_item(&selected_dest, item)?;
            }
            self.add_prompt_state = None;
            self.status_message = Some(format!("Added {} items to '{}'", count, selected_dest));
            return Ok(());
        }
```

- [ ] **Step 2: Add tokio import to library.rs if not present**

Check if `tokio` is imported. If not, add:
```rust
use std::sync::Arc;
```
This is already imported. For `tokio::runtime::Runtime`, add at the top of library.rs:
```rust
use tokio::runtime::Runtime;
```

Actually, the existing code uses `tokio::runtime::Runtime::new()` inline. So just use it the same way.

- [ ] **Step 3: Run cargo check**

```bash
cargo check 2>&1
```

Fix any remaining compilation errors.

- [ ] **Step 4: Run cargo fmt**

```bash
cargo fmt
```

- [ ] **Step 5: Run cargo clippy**

```bash
cargo clippy -- -D warnings 2>&1
```

Fix any warnings.

- [ ] **Step 6: Run all tests**

```bash
cargo test 2>&1
```

- [ ] **Step 7: Re-read modified files to verify consistency**

Check that all match arms are exhaustive, all new methods are called, and no dead code exists.

- [ ] **Step 8: Commit**

```bash
git add src/screens/library.rs src/app.rs
git commit -m "feat: add-all candidates and playlist create/rename via search bar"
```

---

### Task 10: Integration smoke test (manual)

No code changes. Just verify the build compiles and runs:

- [ ] **Step 1: Build release**

```bash
cargo build --release 2>&1
```

- [ ] **Step 2: Verify binary exists**

```bash
ls -la target/release/biu-tui
```
