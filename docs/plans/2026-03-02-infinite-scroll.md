# Infinite Scroll Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add infinite scroll to favorite folder contents and history page, automatically loading more items when the cursor reaches the bottom.

**Architecture:** Add pagination state to LibraryScreen, modify next_item() to trigger loading when at the last item, and implement load-more methods for both favorites and history.

**Tech Stack:** Rust, Tokio async runtime, Ratatui

---

### Task 1: Add Pagination State to LibraryScreen

**Files:**
- Modify: `src/screens/library.rs:44-57`

**Step 1: Add pagination fields to LibraryScreen struct**

```rust
#[derive(Clone)]
pub struct LibraryScreen {
    pub current_tab: LibraryTab,
    pub folders: Vec<FavoriteFolder>,
    pub resources: Vec<FavoriteResource>,
    pub episodes: Vec<crate::api::VideoPage>,
    pub watch_later: Vec<WatchLaterItem>,
    pub history: Vec<HistoryItem>,
    pub list_state: ListState,
    pub nav_level: NavigationLevel,
    pub now_playing: Option<(String, String)>,
    pub current_video_info: Option<crate::api::VideoInfo>,
    pub loop_mode: LoopMode,
    pub status_message: Option<String>,
    // Pagination state for favorite folder resources
    pub current_folder_page: u32,
    pub has_more_resources: bool,
    // Pagination state for history
    pub history_page: u32,
    pub has_more_history: bool,
    // Loading lock
    pub is_loading_more: bool,
}
```

**Step 2: Initialize pagination fields in LibraryScreen::new()**

```rust
impl LibraryScreen {
    pub fn new() -> Self {
        Self {
            current_tab: LibraryTab::Favorites,
            folders: Vec::new(),
            resources: Vec::new(),
            episodes: Vec::new(),
            watch_later: Vec::new(),
            history: Vec::new(),
            list_state: ListState::default(),
            nav_level: NavigationLevel::Folders,
            now_playing: None,
            current_video_info: None,
            loop_mode: LoopMode::default(),
            status_message: None,
            current_folder_page: 1,
            has_more_resources: true,
            history_page: 1,
            has_more_history: true,
            is_loading_more: false,
        }
    }
}
```

**Step 3: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 4: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add pagination state to LibraryScreen"
```

---

### Task 2: Implement try_load_more_favorites Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add try_load_more_favorites method to LibraryScreen**

Add this method after the `go_back()` method (around line 903):

```rust
fn try_load_more_favorites(&mut self, client: Arc<Mutex<BilibiliClient>>, folder_id: u64) {
    if !self.has_more_resources || self.is_loading_more {
        return;
    }
    
    self.is_loading_more = true;
    self.status_message = Some("Loading more...".to_string());
    
    let next_page = self.current_folder_page + 1;
    
    let result = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new().ok();
        rt.and_then(|rt| rt.block_on(client.get_folder_resources(folder_id, next_page)).ok())
    };
    
    match result {
        Some((new_resources, has_more)) => {
            self.resources.extend(new_resources);
            self.current_folder_page = next_page;
            self.has_more_resources = has_more;
            self.status_message = None;
        }
        None => {
            self.status_message = Some("Failed to load more".to_string());
        }
    }
    
    self.is_loading_more = false;
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement try_load_more_favorites method"
```

---

### Task 3: Implement try_load_more_history Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add try_load_more_history method to LibraryScreen**

Add this method after `try_load_more_favorites`:

```rust
fn try_load_more_history(&mut self, client: Arc<Mutex<BilibiliClient>>) {
    if !self.has_more_history || self.is_loading_more {
        return;
    }
    
    self.is_loading_more = true;
    self.status_message = Some("Loading more...".to_string());
    
    let next_page = self.history_page + 1;
    
    let result = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new().ok();
        rt.and_then(|rt| rt.block_on(client.get_history(next_page)).ok())
    };
    
    match result {
        Some(new_history) => {
            self.has_more_history = new_history.len() >= 20;
            self.history.extend(new_history);
            self.history_page = next_page;
            self.status_message = None;
        }
        None => {
            self.status_message = Some("Failed to load more".to_string());
        }
    }
    
    self.is_loading_more = false;
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement try_load_more_history method"
```

---

### Task 4: Implement try_load_more Dispatcher Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add try_load_more method to LibraryScreen**

Add this method before `try_load_more_favorites`:

```rust
fn try_load_more(&mut self, client: Arc<Mutex<BilibiliClient>>) {
    if self.is_loading_more {
        return;
    }
    
    match self.current_tab {
        LibraryTab::Favorites => {
            if let NavigationLevel::Videos { folder_id, .. } = &self.nav_level {
                self.try_load_more_favorites(client, *folder_id);
            }
        }
        LibraryTab::History => self.try_load_more_history(client),
        _ => {}
    }
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement try_load_more dispatcher method"
```

---

### Task 5: Modify next_item to Trigger Load More

**Files:**
- Modify: `src/screens/library.rs:378-387`

**Step 1: Update next_item signature and implementation**

Replace the existing `next_item` method:

```rust
pub fn next_item(
    &mut self,
    playing_list: &Arc<Mutex<PlayingListManager>>,
    client: Arc<Mutex<BilibiliClient>>,
) {
    let len = self.current_list_len_with_playlist(playing_list);
    if len > 0 {
        let current = self.list_state.selected().unwrap_or(0);
        let next_idx = if current >= len - 1 { 0 } else { current + 1 };
        self.list_state.select(Some(next_idx));
        
        if next_idx == len - 1 {
            self.try_load_more(client);
        }
    }
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: Compilation errors about wrong number of arguments in app.rs

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: modify next_item to trigger load more"
```

---

### Task 6: Update App.rs to Pass Client to next_item

**Files:**
- Modify: `src/app.rs:202-204`

**Step 1: Update the next_item call in handle_key**

Replace:
```rust
KeyCode::Char('j') | KeyCode::Down => {
    library.next_item(&self.playing_list);
}
```

With:
```rust
KeyCode::Char('j') | KeyCode::Down => {
    library.next_item(&self.playing_list, self.client.clone());
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: pass client to next_item in app.rs"
```

---

### Task 7: Reset Pagination in select_folder

**Files:**
- Modify: `src/screens/library.rs:492-514`

**Step 1: Update select_folder to reset pagination**

Update the `select_folder` method:

```rust
fn select_folder(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
    if let Some(idx) = self.list_state.selected() {
        if idx < self.folders.len() {
            let folder = &self.folders[idx];
            let folder_id = folder.id;
            let folder_title = folder.title.clone();

            self.current_folder_page = 1;
            
            let resources = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(client.get_folder_resources(folder_id, 1))?
            };

            self.resources = resources.0;
            self.has_more_resources = resources.1;
            self.nav_level = NavigationLevel::Videos {
                folder_id,
                folder_title,
            };
            self.list_state.select(Some(0));
        }
    }
    Ok(())
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: reset pagination when selecting a new folder"
```

---

### Task 8: Initialize History Pagination in load_data

**Files:**
- Modify: `src/screens/library.rs:137-171`

**Step 1: Update load_data to initialize history pagination**

Add pagination initialization at the end of `load_data`:

```rust
pub fn load_data(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let mid = { client.lock().mid };

    let folders = if let Some(mid) = mid {
        let c = client.lock();
        rt.block_on(c.get_created_folders(mid))
            .map_err(|e| anyhow::anyhow!("Favorites API failed: {}", e))?
    } else {
        Vec::new()
    };

    let watch_later = {
        let c = client.lock();
        rt.block_on(c.get_watch_later())
            .map_err(|e| anyhow::anyhow!("Watch Later API failed: {}", e))?
    };

    let history = {
        let c = client.lock();
        rt.block_on(c.get_history(1))
            .map_err(|e| anyhow::anyhow!("History API failed: {}", e))?
    };

    self.folders = folders;
    self.watch_later = watch_later;
    self.history = history;
    
    // Initialize history pagination
    self.history_page = 1;
    self.has_more_history = self.history.len() >= 20;

    // Initialize selection to the first item if data was loaded
    if !self.folders.is_empty() {
        self.list_state.select(Some(0));
    }

    Ok(())
}
```

**Step 2: Run cargo check to verify**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: initialize history pagination in load_data"
```

---

### Task 9: Run Full Build and Lint

**Files:**
- None (verification only)

**Step 1: Run cargo build**

Run: `cargo build`
Expected: Build succeeds with no errors

**Step 2: Run cargo clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 3: Run cargo fmt**

Run: `cargo fmt`

**Step 4: Final commit if any formatting changes**

```bash
git add -A
git commit -m "chore: format code" || echo "No formatting changes"
```

---

### Task 10: Manual Testing

**Files:**
- None (manual testing)

**Step 1: Build and run the application**

Run: `cargo run --release`

**Step 2: Test favorite folder infinite scroll**

1. Navigate to Favorites tab
2. Enter a folder with more than 20 items
3. Navigate to the bottom of the list using 'j' or Down arrow
4. Verify "Loading more..." appears and more items load
5. Verify no duplicate loading occurs

**Step 3: Test history infinite scroll**

1. Navigate to History tab
2. Navigate to the bottom of the list using 'j' or Down arrow
3. Verify "Loading more..." appears and more items load

**Step 4: Test pagination reset**

1. Enter a folder, scroll down to trigger load
2. Press Escape to go back
3. Enter a different folder
4. Verify it starts from page 1

**Step 5: Mark complete**

If all tests pass, the implementation is complete.
