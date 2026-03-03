# Search Functionality Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add search functionality to biu-tui with "/" key activation, supporting API and local search across all library tabs and episodes.

**Architecture:** Mode-based overlay system with SearchState managing query and results. Each screen maintains search state, shows overlay search bar, and filters/restores lists. Uses Bilibili search APIs for server data, local filtering for client data.

**Tech Stack:** Rust, Ratatui, Tokio, reqwest, anyhow

---

## Task 1: Create Search State Module

**Files:**
- Create: `src/screens/search.rs`
- Modify: `src/screens/mod.rs`

**Step 1: Create search.rs with SearchState struct**

Create `src/screens/search.rs`:

```rust
use crate::api::{FavoriteFolder, FavoriteResource, HistoryItem, VideoPage, WatchLaterItem};
use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub cursor_position: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            is_active: true,
            cursor_position: 0,
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor_position = 0;
    }

    pub fn push_char(&mut self, c: char) {
        self.query.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn pop_char(&mut self) -> bool {
        if self.cursor_position > 0 {
            self.query.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Searchable {
    fn matches(&self, query: &str) -> bool;
}

impl Searchable for FavoriteFolder {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for FavoriteResource {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
            || self.upper.name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for VideoPage {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.part.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for WatchLaterItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let owner_name = self.owner.as_ref().map(|o| o.name.as_str()).unwrap_or("");
        self.title.to_lowercase().contains(&query_lower)
            || owner_name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for HistoryItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let owner_name = self.owner.as_ref().map(|o| o.name.as_str()).unwrap_or("");
        self.title.to_lowercase().contains(&query_lower)
            || owner_name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for PlaylistItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
            || self.artist.to_lowercase().contains(&query_lower)
    }
}
```

**Step 2: Export search module**

Modify `src/screens/mod.rs`, add to exports:

```rust
pub mod search;

pub use search::{SearchState, Searchable};
```

**Step 3: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 4: Commit**

```bash
git add src/screens/search.rs src/screens/mod.rs
git commit -m "feat: add search state and searchable trait"
```

---

## Task 2: Create Search Bar Widget

**Files:**
- Create: `src/ui/widgets/search_bar.rs`
- Modify: `src/ui/widgets/mod.rs`

**Step 1: Create search_bar.rs widget**

Create `src/ui/widgets/search_bar.rs`:

```rust
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub struct SearchBar<'a> {
    query: &'a str,
    cursor_position: usize,
}

impl<'a> SearchBar<'a> {
    pub fn new(query: &'a str, cursor_position: usize) -> Self {
        Self {
            query,
            cursor_position,
        }
    }

    pub fn render(self, f: &mut Frame, area: Rect) {
        let mut display_text = format!("Search: {}", self.query);
        
        if area.width as usize > display_text.len() + 1 {
            display_text.push('_');
        }

        let search_bar = Paragraph::new(display_text)
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan)),
            );

        f.render_widget(search_bar, area);
    }
}
```

**Step 2: Export search bar widget**

Modify `src/ui/widgets/mod.rs`, add:

```rust
pub mod search_bar;

pub use search_bar::SearchBar;
```

**Step 3: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 4: Commit**

```bash
git add src/ui/widgets/search_bar.rs src/ui/widgets/mod.rs
git commit -m "feat: add search bar widget"
```

---

## Task 3: Add Search API Methods

**Files:**
- Modify: `src/api/client.rs`
- Modify: `src/api/favorite.rs`

**Step 1: Add search_folder_resources method**

Modify `src/api/client.rs`, add method:

```rust
pub async fn search_folder_resources(
    &self,
    media_id: u64,
    keyword: &str,
) -> Result<Vec<FavoriteResource>> {
    let path = format!(
        "/x/v3/fav/resource/list?media_id={}&keyword={}&ps=100&pn=1",
        media_id,
        urlencoding::encode(keyword)
    );
    let response: ApiResponse<FavoriteResourceListData> = self.get(&path).await?;
    Ok(response.data.map(|d| d.medias).unwrap_or_default())
}
```

**Step 2: Add search_watch_later method**

Modify `src/api/client.rs`, add method:

```rust
pub async fn search_watch_later(&self, keyword: &str) -> Result<Vec<WatchLaterItem>> {
    let path = format!(
        "/x/v2/history/toview/web?key={}&ps=100&pn=1",
        urlencoding::encode(keyword)
    );
    let response: ApiResponse<WatchLaterListData> = self.get(&path).await?;
    Ok(response.data.map(|d| d.list).unwrap_or_default())
}
```

**Step 3: Add search_history method**

Modify `src/api/client.rs`, add method:

```rust
pub async fn search_history(&self, keyword: &str) -> Result<Vec<HistoryItem>> {
    let path = format!(
        "/x/web-interface/history/search?keyword={}&ps=100&pn=1",
        urlencoding::encode(keyword)
    );
    let response: ApiResponse<HistorySearchData> = self.get(&path).await?;
    Ok(response.data.map(|d| d.list).unwrap_or_default())
}
```

**Step 4: Add urlencoding dependency**

Modify `Cargo.toml`, add to dependencies:

```toml
urlencoding = "2.1"
```

**Step 5: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 6: Commit**

```bash
git add src/api/client.rs Cargo.toml
git commit -m "feat: add search API methods for favorites, watch later, history"
```

---

## Task 4: Add Search State to Library Screen

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add search_state field to LibraryScreen**

Modify `src/screens/library.rs`, add field to struct (around line 46):

```rust
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
    pub current_folder_page: u32,
    pub has_more_resources: bool,
    pub history_page: u32,
    pub has_more_history: bool,
    pub is_loading_more: bool,
    visible_height: usize,
    pub search_state: Option<crate::screens::SearchState>,
    pub original_folders: Option<Vec<FavoriteFolder>>,
    pub original_resources: Option<Vec<FavoriteResource>>,
    pub original_episodes: Option<Vec<crate::api::VideoPage>>,
    pub original_watch_later: Option<Vec<WatchLaterItem>>,
    pub original_history: Option<Vec<HistoryItem>>,
}
```

**Step 2: Initialize new fields in new()**

Modify `src/screens/library.rs` in `new()` method (around line 74):

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
        loop_mode: LoopMode::default(),
        status_message: None,
        current_folder_page: 1,
        has_more_resources: true,
        history_page: 1,
        has_more_history: true,
        is_loading_more: false,
        visible_height: 0,
        search_state: None,
        original_folders: None,
        original_resources: None,
        original_episodes: None,
        original_watch_later: None,
        original_history: None,
    }
}
```

**Step 3: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 4: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: add search state fields to LibraryScreen"
```

---

## Task 5: Implement Search Mode Activation

**Files:**
- Modify: `src/app.rs`

**Step 1: Add enter_search_mode method**

Modify `src/app.rs`, add method after `handle_key`:

```rust
fn enter_search_mode(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        library.search_state = Some(crate::screens::SearchState::new());
        
        library.original_folders = Some(library.folders.clone());
        library.original_resources = Some(library.resources.clone());
        library.original_episodes = Some(library.episodes.clone());
        library.original_watch_later = Some(library.watch_later.clone());
        library.original_history = Some(library.history.clone());
    }
    Ok(())
}
```

**Step 2: Add exit_search_mode method**

Modify `src/app.rs`, add method:

```rust
fn exit_search_mode(&mut self, restore: bool) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        if restore {
            if let Some(original) = library.original_folders.take() {
                library.folders = original;
            }
            if let Some(original) = library.original_resources.take() {
                library.resources = original;
            }
            if let Some(original) = library.original_episodes.take() {
                library.episodes = original;
            }
            if let Some(original) = library.original_watch_later.take() {
                library.watch_later = original;
            }
            if let Some(original) = library.original_history.take() {
                library.history = original;
            }
        }
        
        library.search_state = None;
        library.original_folders = None;
        library.original_resources = None;
        library.original_episodes = None;
        library.original_watch_later = None;
        library.original_history = None;
    }
    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add search mode activation and exit methods"
```

---

## Task 6: Add "/" Key Binding

**Files:**
- Modify: `src/app.rs`

**Step 1: Handle "/" key in Library screen**

Modify `src/app.rs` in `handle_key` method, in the Library branch (around line 207), add before other key handlers:

```rust
KeyCode::Char('/') => {
    if library.search_state.is_none() {
        self.enter_search_mode()?;
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add / key binding to enter search mode"
```

---

## Task 7: Implement Search Query Input Handling

**Files:**
- Modify: `src/app.rs`

**Step 1: Add search input handling**

Modify `src/app.rs` in `handle_key` method, in the Library branch, add at the beginning:

```rust
if let Some(ref mut search_state) = library.search_state {
    match code {
        KeyCode::Esc => {
            self.exit_search_mode(true)?;
            return Ok(());
        }
        KeyCode::Enter => {
            self.exit_search_mode(false)?;
            return Ok(());
        }
        KeyCode::Char('u') if _modifiers.contains(KeyModifiers::CONTROL) => {
            search_state.clear();
            self.perform_search()?;
            return Ok(());
        }
        KeyCode::Backspace => {
            search_state.pop_char();
            self.perform_search()?;
            return Ok(());
        }
        KeyCode::Char(c) => {
            search_state.push_char(*c);
            self.perform_search()?;
            return Ok(());
        }
        _ => {}
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: add search query input handling"
```

---

## Task 8: Implement Search Execution

**Files:**
- Modify: `src/app.rs`

**Step 1: Add perform_search method**

Modify `src/app.rs`, add method:

```rust
fn perform_search(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        if let Some(ref search_state) = library.search_state {
            let query = search_state.query.trim();
            
            if query.is_empty() {
                if let Some(original) = &library.original_folders {
                    library.folders = original.clone();
                }
                if let Some(original) = &library.original_resources {
                    library.resources = original.clone();
                }
                if let Some(original) = &library.original_episodes {
                    library.episodes = original.clone();
                }
                if let Some(original) = &library.original_watch_later {
                    library.watch_later = original.clone();
                }
                if let Some(original) = &library.original_history {
                    library.history = original.clone();
                }
                return Ok(());
            }

            match library.current_tab {
                LibraryTab::Favorites => {
                    match &library.nav_level {
                        NavigationLevel::Folders => {
                            if let Some(original) = &library.original_folders {
                                library.folders = original
                                    .iter()
                                    .filter(|f| f.matches(query))
                                    .cloned()
                                    .collect();
                            }
                        }
                        NavigationLevel::Videos { folder_id, .. } => {
                            let folder_id = *folder_id;
                            let client = self.client.clone();
                            let keyword = query.to_string();
                            
                            let rt = tokio::runtime::Runtime::new()?;
                            let results = rt.block_on(async {
                                let c = client.lock();
                                c.search_folder_resources(folder_id, &keyword).await
                            });
                            
                            match results {
                                Ok(resources) => {
                                    library.resources = resources;
                                }
                                Err(e) => {
                                    library.status_message = Some(format!("Search failed: {}", e));
                                }
                            }
                        }
                        NavigationLevel::Episodes { .. } => {
                            if let Some(original) = &library.original_episodes {
                                library.episodes = original
                                    .iter()
                                    .filter(|ep| ep.matches(query))
                                    .cloned()
                                    .collect();
                            }
                        }
                    }
                }
                LibraryTab::WatchLater => {
                    let client = self.client.clone();
                    let keyword = query.to_string();
                    
                    let rt = tokio::runtime::Runtime::new()?;
                    let results = rt.block_on(async {
                        let c = client.lock();
                        c.search_watch_later(&keyword).await
                    });
                    
                    match results {
                        Ok(items) => {
                            library.watch_later = items;
                        }
                        Err(e) => {
                            library.status_message = Some(format!("Search failed: {}", e));
                        }
                    }
                }
                LibraryTab::History => {
                    let client = self.client.clone();
                    let keyword = query.to_string();
                    
                    let rt = tokio::runtime::Runtime::new()?;
                    let results = rt.block_on(async {
                        let c = client.lock();
                        c.search_history(&keyword).await
                    });
                    
                    match results {
                        Ok(items) => {
                            library.history = items;
                        }
                        Err(e) => {
                            library.status_message = Some(format!("Search failed: {}", e));
                        }
                    }
                }
                LibraryTab::PlayingList => {
                    let playing_list = self.playing_list.lock();
                    let items = playing_list.items();
                    library.folders = items
                        .iter()
                        .filter(|item| item.matches(query))
                        .map(|item| FavoriteFolder {
                            id: 0,
                            title: format!("{} - {}", item.title, item.artist),
                            media_count: 1,
                        })
                        .collect();
                }
            }
            
            library.list_state.select(Some(0));
        }
    }
    Ok(())
}
```

**Step 2: Add use statement for Searchable trait**

Modify `src/app.rs`, add at top:

```rust
use crate::screens::Searchable;
```

**Step 3: Verify compilation**

Run: `cargo check`

Expected: Compilation errors about missing API types

**Step 4: Add missing API types**

Modify `src/api/types.rs`, add response types:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteResourceListData {
    pub medias: Vec<FavoriteResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchLaterListData {
    pub list: Vec<WatchLaterItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistorySearchData {
    pub list: Vec<HistoryItem>,
}
```

**Step 5: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 6: Commit**

```bash
git add src/app.rs src/api/types.rs
git commit -m "feat: implement search execution for all tabs"
```

---

## Task 9: Render Search Bar Overlay

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Import SearchBar widget**

Modify `src/screens/library.rs`, add to imports:

```rust
use crate::ui::widgets::SearchBar;
use crate::screens::SearchState;
```

**Step 2: Update render method to show search bar**

Modify `src/screens/library.rs` in `render` method, update layout constraints (around line 198):

```rust
let mut constraints = vec![
    Constraint::Length(3),
    Constraint::Length(1),
];

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
```

**Step 3: Render search bar widget**

Modify `src/screens/library.rs` in `render` method, after breadcrumb rendering:

```rust
let mut chunk_offset = 2;

if let Some(ref search_state) = self.search_state {
    SearchBar::new(&search_state.query, search_state.cursor_position)
        .render(f, chunks[chunk_offset]);
    chunk_offset += 1;
}
```

**Step 4: Update list rendering to use correct chunk**

Modify `src/screens/library.rs` in `render` method, update list rendering:

```rust
let visible_height = chunks[chunk_offset].height.saturating_sub(2) as usize;
self.adjust_scroll_offset(visible_height);
f.render_stateful_widget(list, chunks[chunk_offset], &mut self.list_state);
self.visible_height = visible_height;
```

**Step 5: Update remaining chunks**

Modify `src/screens/library.rs` in `render` method, update remaining chunk references:

```rust
chunk_offset += 1;

let now_playing_text = if let Some((title, artist)) = &self.now_playing {
    format!("♫ Now Playing: {} - {}", title, artist)
} else {
    "♫ Not Playing".to_string()
};
let now_playing = Paragraph::new(now_playing_text)
    .style(Style::default().fg(Color::Cyan))
    .block(Block::default().borders(Borders::TOP));
f.render_widget(now_playing, chunks[chunk_offset]);

chunk_offset += 1;

// ... progress bar and help text use chunks[chunk_offset] and chunks[chunk_offset + 1]
```

**Step 6: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 7: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: render search bar overlay in library screen"
```

---

## Task 10: Update Help Text for Search Mode

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Update help text logic**

Modify `src/screens/library.rs` in `render` method, update help text (around line 381):

```rust
let (help_text, help_style) = if let Some(msg) = &self.status_message {
    (msg.clone(), Style::default().fg(Color::Green))
} else if self.search_state.is_some() {
    (
        "[/] Search  [Esc] Exit search  [Enter] Keep results  [Ctrl+U] Clear".to_string(),
        Style::default(),
    )
} else {
    let text = match self.current_tab {
        LibraryTab::PlayingList => {
            "[j/k] Navigate  [Enter] Jump  [d] Remove  [Tab] Switch  [/] Search"
        }
        _ => {
            "[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [a] Add to list  [A] Add all  [Tab] Switch  [/] Search"
        }
    };
    (text.to_string(), Style::default())
};
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: update help text to show search keybindings"
```

---

## Task 11: Handle Tab Switching During Search

**Files:**
- Modify: `src/app.rs`

**Step 1: Clear search state on tab switch**

Modify `src/app.rs` in `handle_key` method, in the Tab key handling (around line 212):

```rust
KeyCode::Tab => {
    if library.search_state.is_some() {
        self.exit_search_mode(true)?;
    }
    library.current_tab = match library.current_tab {
        LibraryTab::Favorites => LibraryTab::WatchLater,
        LibraryTab::WatchLater => LibraryTab::History,
        LibraryTab::History => LibraryTab::PlayingList,
        LibraryTab::PlayingList => LibraryTab::Favorites,
    };
    library.reset_selection_for_tab(self.playing_list.clone());
}
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: clear search state when switching tabs"
```

---

## Task 12: Add Debouncing for API Searches

**Files:**
- Modify: `src/app.rs`

**Step 1: Add last_search_time field to App**

Modify `src/app.rs`, add field to App struct (around line 26):

```rust
pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
    screen: Screen,
    client: Arc<Mutex<BilibiliClient>>,
    player: Option<AudioPlayer>,
    _downloader: Option<DownloadManager>,
    _config: Config,
    last_qr_poll: Option<Instant>,
    settings: Settings,
    playing_list: Arc<Mutex<PlayingListManager>>,
    previous_library: Option<LibraryScreen>,
    previous_player_state: Option<PlayerState>,
    mpris: Option<MprisManager>,
    last_search_time: Option<Instant>,
}
```

**Step 2: Initialize field**

Modify `src/app.rs` in `init_app_state` method (around line 104):

```rust
Ok(Self {
    terminal,
    running: true,
    screen,
    client,
    player: None,
    _downloader: None,
    _config: config,
    last_qr_poll: None,
    settings,
    playing_list,
    previous_library: None,
    previous_player_state: None,
    mpris,
    last_search_time: None,
})
```

**Step 3: Update perform_search with debouncing**

Modify `src/app.rs` in `perform_search` method, add debouncing logic:

```rust
fn perform_search(&mut self) -> Result<()> {
    if let Screen::Library(library) = &mut self.screen {
        if let Some(ref search_state) = library.search_state {
            let query = search_state.query.trim();
            
            let needs_api_call = matches!(
                library.current_tab,
                LibraryTab::Favorites if matches!(library.nav_level, NavigationLevel::Videos { .. })
            ) || matches!(
                library.current_tab,
                LibraryTab::WatchLater | LibraryTab::History
            );
            
            if needs_api_call {
                let should_search = self
                    .last_search_time
                    .map(|last| last.elapsed() >= Duration::from_millis(300))
                    .unwrap_or(true);
                
                if !should_search {
                    return Ok(());
                }
                
                self.last_search_time = Some(Instant::now());
            }
            
            // ... rest of search logic
        }
    }
    Ok(())
}
```

**Step 4: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add 300ms debouncing for API searches"
```

---

## Task 13: Handle Empty Search Results

**Files:**
- Modify: `src/screens/library.rs`

**Step 1: Add empty results message in render**

Modify `src/screens/library.rs` in `render` method, after building items list (around line 318):

```rust
let items: Vec<ListItem> = match self.current_tab {
    // ... existing match arms
};

let list = if items.is_empty() && self.search_state.is_some() {
    let query = self.search_state.as_ref().unwrap().query.as_str();
    List::new(vec![ListItem::new(format!(
        "No results found for '{}'",
        query
    ))])
    .block(Block::default().borders(Borders::ALL))
} else {
    List::new(items)
        .block(Block::default().borders(Borders::ALL))
        .highlight_style(Style::default().bg(Color::DarkGray))
};
```

**Step 2: Verify compilation**

Run: `cargo check`

Expected: No errors

**Step 3: Commit**

```bash
git add src/screens/library.rs
git commit -m "feat: show empty results message when search finds nothing"
```

---

## Task 14: Write Unit Tests

**Files:**
- Modify: `src/screens/search.rs`

**Step 1: Add tests module**

Add to end of `src/screens/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_state_push_char() {
        let mut state = SearchState::new();
        state.push_char('a');
        assert_eq!(state.query, "a");
        assert_eq!(state.cursor_position, 1);
        
        state.push_char('b');
        assert_eq!(state.query, "ab");
        assert_eq!(state.cursor_position, 2);
    }

    #[test]
    fn test_search_state_pop_char() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('b');
        assert!(state.pop_char());
        assert_eq!(state.query, "a");
        assert_eq!(state.cursor_position, 1);
    }

    #[test]
    fn test_search_state_clear() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('b');
        state.clear();
        assert!(state.is_empty());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_searchable_favorite_folder() {
        let folder = FavoriteFolder {
            id: 1,
            title: "Music Videos".to_string(),
            media_count: 10,
        };
        
        assert!(folder.matches("music"));
        assert!(folder.matches("VIDEO"));
        assert!(!folder.matches("games"));
    }

    #[test]
    fn test_searchable_favorite_resource() {
        let resource = FavoriteResource {
            bvid: "BV123".to_string(),
            title: "Test Video".to_string(),
            upper: crate::api::Owner {
                mid: 1,
                name: "TestUser".to_string(),
                face: String::new(),
            },
            duration: 100,
        };
        
        assert!(resource.matches("test"));
        assert!(resource.matches("USER"));
        assert!(!resource.matches("other"));
    }

    #[test]
    fn test_searchable_case_insensitive() {
        let folder = FavoriteFolder {
            id: 1,
            title: "Music".to_string(),
            media_count: 5,
        };
        
        assert!(folder.matches("MUSIC"));
        assert!(folder.matches("music"));
        assert!(folder.matches("MUsIC"));
    }
}
```

**Step 2: Run tests**

Run: `cargo test`

Expected: All tests pass

**Step 3: Commit**

```bash
git add src/screens/search.rs
git commit -m "test: add unit tests for search state and searchable trait"
```

---

## Task 15: Manual Testing

**Step 1: Build and run**

Run: `cargo run --release`

**Step 2: Test search activation**

- Press "/" in Library screen
- Verify search bar appears
- Verify help text updates

**Step 3: Test search in each tab**

- Favorites tab: Search folders, then enter folder and search videos
- Watch Later tab: Search with API
- History tab: Search with API
- Playing List tab: Search locally

**Step 4: Test search navigation**

- Type query and verify list filters
- Use j/k to navigate results
- Press Esc to exit and restore list
- Press Enter to exit and keep results

**Step 5: Test edge cases**

- Empty search query
- No results found
- Special characters in query
- Switch tabs during search

**Step 6: Document any issues**

Create GitHub issues for any bugs found.

---

## Success Criteria

- [ ] "/" activates search in all library tabs
- [ ] Search bar overlay appears correctly
- [ ] API searches work (Favorites videos, Watch Later, History)
- [ ] Local filtering works (Favorites folders, Episodes, Playing List)
- [ ] Esc restores original list
- [ ] Enter keeps filtered results
- [ ] j/k navigation works on filtered results
- [ ] Empty results show "No results found" message
- [ ] Case-insensitive search works
- [ ] 300ms debouncing prevents excessive API calls
- [ ] Tab switching clears search state
- [ ] Help text updates for search mode
- [ ] All unit tests pass
- [ ] Manual testing completes without critical bugs
