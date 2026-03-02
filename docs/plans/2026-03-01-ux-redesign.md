# UX Redesign Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add breadcrumb navigation, episode list support, ESC navigation, and progress bar to biu-tui.

**Architecture:** Extend LibraryScreen with navigation state tracking, add new UI components (breadcrumb bar, progress bar), and update key handling for hierarchical navigation.

**Tech Stack:** Rust, Ratatui, parking_lot::Mutex

---

## Task 1: Add Navigation State Types

**Files:**
- Modify: `src/screens/library.rs:13-30`

**Step 1: Add NavigationLevel enum and expand LibraryScreen state**

Add after `LibraryTab` enum (line 18):

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum NavigationLevel {
    Folders,
    Videos { folder_id: u64, folder_title: String },
    Episodes { folder_id: u64, folder_id_title: String, bvid: String, video_title: String },
}
```

**Step 2: Update LibraryScreen struct**

Replace the struct (lines 20-30) with:

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
}
```

**Step 3: Update LibraryScreen::new()**

Replace `new()` function (lines 33-44) with:

```rust
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
    }
}
```

**Step 4: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 5: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add NavigationLevel enum and expand LibraryScreen state"
```

---

## Task 2: Add Breadcrumb Rendering

**Files:**
- Modify: `src/screens/library.rs:86-95`

**Step 1: Add breadcrumb helper function**

Add after `format_duration` function (line 322):

```rust
fn format_time(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}
```

**Step 2: Add breadcrumb rendering in render()**

Modify the `render()` method. Add a new chunk for breadcrumb between tabs and list. Update the layout constraints (lines 87-95):

```rust
let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(10),
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .split(area);
```

**Step 3: Add breadcrumb widget rendering**

After tabs rendering (line 103), add breadcrumb:

```rust
let breadcrumb_text = match &self.nav_level {
    NavigationLevel::Folders => "Favorites".to_string(),
    NavigationLevel::Videos { folder_title, .. } => {
        format!("Favorites > {}", folder_title)
    }
    NavigationLevel::Episodes { folder_id_title, video_title, .. } => {
        format!("Favorites > {} > {}", folder_id_title, video_title)
    }
};
let breadcrumb = Paragraph::new(breadcrumb_text)
    .style(Style::default().fg(Color::Yellow));
f.render_widget(breadcrumb, chunks[1]);
```

**Step 4: Update list widget to use chunks[2]**

Change `chunks[1]` to `chunks[2]` for the list widget (line 162).

**Step 5: Update now-playing to use chunks[3]**

Change `chunks[2]` to `chunks[3]` for now-playing (line 172).

**Step 6: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 7: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add breadcrumb bar rendering"
```

---

## Task 3: Add Progress Bar Rendering

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add progress bar rendering after now-playing**

After now-playing rendering (around line 173), add progress bar:

```rust
let progress_text = if let Some(p) = player {
    let pos = p.position();
    let dur = p.duration();
    let pos_str = format_time(pos);
    let dur_str = format_time(dur);
    
    let progress = if dur.as_secs() > 0 {
        pos.as_secs_f32() / dur.as_secs_f32()
    } else {
        0.0
    };
    
    let width = chunks[4].width as usize;
    let bar_width = width.saturating_sub(20);
    let filled = (bar_width as f32 * progress) as usize;
    let filled = filled.min(bar_width);
    
    let bar: String = if bar_width > 0 {
        let filled_chars: String = std::iter::repeat('━').take(filled).collect();
        let empty_chars: String = std::iter::repeat('─').take(bar_width - filled).collect();
        format!("{}╾{}", filled_chars, empty_chars)
    } else {
        String::new()
    };
    
    format!("{}  {} / {}", bar, pos_str, dur_str)
} else {
    "━━──────────────  --:-- / --:--".to_string()
};

let progress_bar = Paragraph::new(progress_text)
    .style(Style::default().fg(Color::Cyan))
    .block(Block::default().borders(Borders::TOP));
f.render_widget(progress_bar, chunks[4]);
```

**Step 2: Update render signature to accept player**

Change the `render` method signature (line 86):

```rust
pub fn render(&mut self, f: &mut Frame, area: Rect, player: Option<&AudioPlayer>)
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Errors in app.rs about render call signature

**Step 4: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add progress bar rendering"
```

---

## Task 4: Update App to Pass Player to Render

**Files:**
- Modify: `src/app.rs:98-107`

**Step 1: Update render call in App::run()**

Replace the library render call (lines 102-106):

```rust
Screen::Library(library) => {
    let mut lib = library.clone();
    lib.render(f, area, self.player.as_ref());
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: pass player reference to library render"
```

---

## Task 5: Update List Rendering for Episodes and Multi-part Indicator

**Files:**
- Modify: `src/screens/library.rs:105-157`

**Step 1: Add helper to check multi-part videos**

Add helper function after `format_time`:

```rust
fn is_multi_part(video_info: &crate::api::VideoInfo) -> bool {
    video_info.pages.len() > 1
}
```

**Step 2: Update Favorites list rendering**

Replace the Favorites list items section (lines 106-127) with:

```rust
LibraryTab::Favorites => {
    match &self.nav_level {
        NavigationLevel::Folders => {
            self.folders
                .iter()
                .map(|f| ListItem::new(format!("{} ({})", f.title, f.media_count)))
                .collect()
        }
        NavigationLevel::Videos { .. } => {
            self.resources
                .iter()
                .map(|r| {
                    let multi_part = if let Some(ref info) = r.page_count {
                        if *info > 1 {
                            format!(" (P1~{})", info)
                        } else {
                            String::new()
                        }
                    } else {
                        String::new()
                    };
                    ListItem::new(format!(
                        "{}{}  {:->5}",
                        r.title,
                        multi_part,
                        format_duration(r.duration)
                    ))
                })
                .collect()
        }
        NavigationLevel::Episodes { .. } => {
            self.episodes
                .iter()
                .map(|p| {
                    ListItem::new(format!(
                        "P{}: {}  {:->5}",
                        p.page,
                        p.part,
                        format_duration(p.duration)
                    ))
                })
                .collect()
        }
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Error - `page_count` field doesn't exist on FavoriteResource

**Step 4: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update list rendering for episodes and multi-part indicator"
```

---

## Task 6: Add page_count to FavoriteResource

**Files:**
- Modify: `src/api/types.rs:25-33`

**Step 1: Add page_count field**

Add to `FavoriteResource` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteResource {
    pub id: u64,
    pub bvid: String,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub upper: Upper,
    #[serde(default)]
    pub page_count: Option<u32>,
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors (page_count will be None if not in API response)

**Step 3: Commit**

```bash
git add src/api/types.rs
git commit -m "feat: add page_count field to FavoriteResource"
```

---

## Task 7: Update current_list_len for Navigation Levels

**Files:**
- Modify: `src/screens/library.rs:201-213`

**Step 1: Update current_list_len method**

Replace the method with:

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
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update current_list_len for navigation levels"
```

---

## Task 8: Update handle_enter for Episode Navigation

**Files:**
- Modify: `src/screens/library.rs:215-233`

**Step 1: Rewrite handle_enter for navigation levels**

Replace the `handle_enter` method:

```rust
pub fn handle_enter(
    &mut self,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
) -> anyhow::Result<()> {
    match self.current_tab {
        LibraryTab::Favorites => {
            match &self.nav_level {
                NavigationLevel::Folders => {
                    self.select_folder(client)?;
                }
                NavigationLevel::Videos { folder_id, folder_title } => {
                    let folder_id = *folder_id;
                    let folder_title = folder_title.clone();
                    self.select_video_or_episodes(client, player, folder_id, folder_title)?;
                }
                NavigationLevel::Episodes { folder_id, folder_id_title, bvid, video_title } => {
                    let bvid = bvid.clone();
                    let video_title = video_title.clone();
                    let folder_id_title = folder_id_title.clone();
                    self.play_selected_episode(client, player, bvid, video_title, folder_id_title, folder_id)?;
                }
            }
        }
        LibraryTab::WatchLater | LibraryTab::History => {
            self.play_selected(client, player)?;
        }
    }
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Errors for missing methods

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update handle_enter for navigation levels"
```

---

## Task 9: Implement select_video_or_episodes Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add select_video_or_episodes method**

Add after `select_folder` method:

```rust
fn select_video_or_episodes(
    &mut self,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
    folder_id: u64,
    folder_title: String,
) -> anyhow::Result<()> {
    if let Some(idx) = self.list_state.selected() {
        if let Some(resource) = self.resources.get(idx).cloned() {
            let bvid = resource.bvid.clone();
            let video_title = resource.title.clone();
            
            let video_info = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(client.get_video_info(&bvid))?
            };
            
            if video_info.pages.len() > 1 {
                self.current_video_info = Some(video_info.clone());
                self.episodes = video_info.pages.clone();
                self.nav_level = NavigationLevel::Episodes {
                    folder_id,
                    folder_id_title: folder_title,
                    bvid,
                    video_title,
                };
                self.list_state.select(Some(0));
            } else {
                self.play_video(client, player, &video_info)?;
            }
        }
    }
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Error for missing play_video method

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement select_video_or_episodes for episode navigation"
```

---

## Task 10: Implement play_video Helper Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add play_video helper method**

Add after `select_video_or_episodes`:

```rust
fn play_video(
    &mut self,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
    video_info: &crate::api::VideoInfo,
) -> anyhow::Result<()> {
    let cid = video_info.cid;
    let bvid = &video_info.bvid;
    
    let audio_stream = {
        let client = client.lock();
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(client.get_best_audio(bvid, cid))?
    };
    
    if player.is_none() {
        *player = Some(AudioPlayer::new()?);
    }
    
    if let Some(p) = player {
        p.play(&audio_stream.url)?;
        self.now_playing = Some((video_info.title.clone(), video_info.owner.name.clone()));
    }
    
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add play_video helper method"
```

---

## Task 11: Implement play_selected_episode Method

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add play_selected_episode method**

Add after `play_video`:

```rust
fn play_selected_episode(
    &mut self,
    client: Arc<Mutex<BilibiliClient>>,
    player: &mut Option<AudioPlayer>,
    bvid: String,
    video_title: String,
    folder_id_title: String,
    folder_id: u64,
) -> anyhow::Result<()> {
    if let Some(idx) = self.list_state.selected() {
        if let Some(episode) = self.episodes.get(idx).cloned() {
            let audio_stream = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(client.get_best_audio(&bvid, episode.cid))?
            };
            
            if player.is_none() {
                *player = Some(AudioPlayer::new()?);
            }
            
            if let Some(p) = player {
                p.play(&audio_stream.url)?;
                let episode_title = if episode.part.is_empty() {
                    video_title.clone()
                } else {
                    format!("{} - P{}: {}", video_title, episode.page, episode.part)
                };
                let uploader = self.current_video_info
                    .as_ref()
                    .map(|i| i.owner.name.clone())
                    .unwrap_or_default();
                self.now_playing = Some((episode_title, uploader));
            }
        }
    }
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: implement play_selected_episode method"
```

---

## Task 12: Update select_folder for Navigation State

**Files:**
- Modify: `src/screens/library.rs:235-252`

**Step 1: Update select_folder method**

Replace the method:

```rust
fn select_folder(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
    if let Some(idx) = self.list_state.selected() {
        if idx < self.folders.len() {
            let folder = &self.folders[idx];
            let folder_id = folder.id;
            let folder_title = folder.title.clone();

            let resources = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                rt.block_on(client.get_folder_resources(folder_id, 1))?
            };

            self.resources = resources.0;
            self.nav_level = NavigationLevel::Videos { folder_id, folder_title };
            self.list_state.select(Some(0));
        }
    }
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update select_folder to set navigation state"
```

---

## Task 13: Update go_back for Hierarchical Navigation

**Files:**
- Modify: `src/screens/library.rs:309-316`

**Step 1: Rewrite go_back method**

Replace the method:

```rust
pub fn go_back(&mut self) {
    match &self.nav_level {
        NavigationLevel::Episodes { folder_id, folder_id_title, .. } => {
            let folder_id = *folder_id;
            let folder_title = folder_id_title.clone();
            self.episodes.clear();
            self.current_video_info = None;
            self.nav_level = NavigationLevel::Videos { folder_id, folder_title };
            self.list_state.select(Some(0));
        }
        NavigationLevel::Videos { .. } => {
            self.resources.clear();
            self.nav_level = NavigationLevel::Folders;
            self.list_state.select(Some(0));
        }
        NavigationLevel::Folders => {
            // Already at root - do nothing, use 'q' to quit
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update go_back for hierarchical navigation"
```

---

## Task 14: Remove ESC Quit Behavior in App

**Files:**
- Modify: `src/app.rs:153`

**Step 1: The ESC behavior is already handled by go_back()**

No changes needed - ESC already calls `library.go_back()` which now handles hierarchy. The `KeyCode::Char('q')` handles quit.

**Step 2: Verify ESC doesn't quit at root**

Run: `cargo run`
Expected: At root level, ESC does nothing, 'q' quits

**Step 3: Commit** (if any changes made)

```bash
git add -A
git commit -m "refactor: ESC only navigates back, 'q' quits"
```

---

## Task 15: Update Help Bar

**Files:**
- Modify: `src/screens/library.rs:174-176`

**Step 1: Update help text**

Replace the help paragraph (around line 174, now using chunks[5]):

```rust
let help = Paragraph::new("[j/k] Navigate  [Enter] Select  [Esc] Back  [Space] Pause  [q] Quit")
    .block(Block::default().borders(Borders::TOP));
f.render_widget(help, chunks[5]);
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update help bar with new key bindings"
```

---

## Task 16: Add Space Key for Play/Pause

**Files:**
- Modify: `src/app.rs:137-155`

**Step 1: Add Space key handler**

Add to the Library match arm (after `KeyCode::Esc`):

```rust
KeyCode::Char(' ') => {
    if let Some(ref player) = self.player {
        match player.state() {
            crate::audio::PlayerState::Playing => player.pause(),
            crate::audio::PlayerState::Paused => player.resume(),
            crate::audio::PlayerState::Stopped => {}
        }
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add Space key for play/pause toggle"
```

---

## Task 17: Update Now Playing State Indicator

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Update now_playing rendering with state**

Update the now_playing rendering section to show state indicator:

```rust
let (state_char, now_playing_text) = if let Some(p) = player {
    match p.state() {
        crate::audio::PlayerState::Playing => {
            if let Some((title, artist)) = &self.now_playing {
                ('♫', format!("Now Playing: {} - {}", title, artist))
            } else {
                ('♫', "Playing...".to_string())
            }
        }
        crate::audio::PlayerState::Paused => {
            if let Some((title, artist)) = &self.now_playing {
                ('⏸', format!("Paused: {} - {}", title, artist))
            } else {
                ('⏸', "Paused".to_string())
            }
        }
        crate::audio::PlayerState::Stopped => {
            ('○', "Not Playing".to_string())
        }
    }
} else {
    ('○', "Not Playing".to_string())
};

let now_playing = Paragraph::now_playing_text)
    .style(Style::default().fg(Color::Cyan))
    .block(Block::default().borders(Borders::TOP));
f.render_widget(now_playing, chunks[3]);
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add state indicator to now playing bar"
```

---

## Task 18: Final Verification

**Step 1: Run full build**

Run: `cargo build --release`
Expected: Build succeeds

**Step 2: Run application**

Run: `cargo run --release`
Manual test:
- Navigate to Favorites tab
- Enter a folder (breadcrumb shows: Favorites > Folder Name)
- Enter a multi-part video (breadcrumb shows: Favorites > Folder > Video)
- ESC goes back one level
- Progress bar shows when playing
- Space pauses/resumes
- 'q' quits

**Step 3: Final commit**

```bash
git add -A
git commit -m "feat: complete UX redesign with breadcrumb, episodes, and progress bar"
```
