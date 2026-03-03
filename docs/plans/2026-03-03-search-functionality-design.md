# Search Functionality Design

**Date:** 2026-03-03  
**Status:** Approved  
**Author:** Design Session

## Overview

Add search functionality to biu-tui allowing users to search through all pages using the "/" key. Search will use Bilibili's search APIs for server-side data and local filtering for client-side data.

## Scope

### Pages with Search
- **Library - Favorites Tab:** Search folders (local) and videos within folders (API)
- **Library - Watch Later Tab:** Search watch later videos (API)
- **Library - History Tab:** Search viewing history (API)
- **Library - Playing List Tab:** Filter playlist items (local)
- **Episodes Page:** Search within a video's episodes (local)

## Architecture

### Mode-Based Overlay System

The search feature implements a **mode-based overlay system** where each screen maintains its own search state.

**Key Components:**
- `SearchState`: Manages search query, results, and mode
- `SearchBar`: UI widget for rendering search input
- `Searchable` trait: Defines search behavior for item types
- API methods: Search endpoints in `BilibiliClient`

### State Management

```rust
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub original_data: Option<Vec<SearchableItem>>,
    pub filtered_results: Vec<SearchableItem>,
    pub cursor_position: usize,
}
```

Each screen maintains `search_state: Option<SearchState>`. When search is active:
- Original data is backed up in `original_data`
- Filtered results stored in `filtered_results`
- Query updates trigger re-filtering from backup

## Data Flow

### Search Activation
1. User presses "/" key
2. Screen enters search mode, shows search bar overlay
3. Original list data backed up to `SearchState::original_data`
4. User types search query

### Search Execution

**Favorites (folders level):** Local filter through folder names  
**Favorites (videos level):** Call `/x/v3/fav/resource/list` with `keyword` parameter  
**Watch Later:** Call `/x/v2/history/toview/web` with `key` parameter  
**History:** Call `/x/web-interface/history/search` with `keyword` parameter  
**Playing List:** Local filter through playlist items (title/artist)  
**Episodes:** Local filter through episode titles

### Search Clear
1. User presses Esc or clears input
2. Search mode exits
3. Original list restored from backup
4. Search bar disappears

## UI/UX Design

### Search Bar Overlay
- **Position:** Top of content area (between tabs and breadcrumb/list)
- **Height:** 3 lines (border, input, border)
- **Style:** Cyan border, white text on dark background
- **Placeholder:** "Search: " followed by cursor
- **Updates:** Real-time with 300ms debounce for API calls

### Visual Feedback
- Search bar always visible when active
- List updates to show only matching results
- Empty results show "No results found" message
- Status bar shows: "Showing X of Y items"

### Key Bindings
- `/` - Enter search mode
- `Esc` - Exit search mode and restore original list
- `Enter` - Exit search mode but keep filtered results
- `Backspace` - Delete character
- Character keys - Add to search query
- `j/k` - Navigate filtered results
- `Ctrl+U` - Clear search query

### Help Text
When search is active:  
`[/] Search  [Esc] Exit search  [Enter] Keep results  [Ctrl+U] Clear`

## API Integration

### New API Methods

```rust
// In api/client.rs
pub async fn search_folder_resources(
    &self, 
    media_id: u64, 
    keyword: &str
) -> Result<Vec<FavoriteResource>>

pub async fn search_watch_later(
    &self, 
    keyword: &str
) -> Result<Vec<WatchLaterItem>>

pub async fn search_history(
    &self, 
    keyword: &str
) -> Result<Vec<HistoryItem>>
```

### API Endpoints
- Favorites: `GET /x/v3/fav/resource/list?keyword={query}`
- Watch Later: `GET /x/v2/history/toview/web?key={query}`
- History: `GET /x/web-interface/history/search?keyword={query}`

### Debouncing Strategy
- **Local searches:** Immediate filtering (no debounce)
- **API searches:** 300ms debounce timer
- Use `std::time::Instant` to track keystrokes

## Data Structures

### Searchable Trait

```rust
pub trait Searchable {
    fn matches(&self, query: &str) -> bool;
}
```

Each item type implements this trait:
- `FavoriteFolder`: matches folder title
- `FavoriteResource`: matches title and UP主 name
- `VideoPage`: matches episode part name
- `WatchLaterItem`: matches title and owner name
- `HistoryItem`: matches title and owner name
- `PlaylistItem`: matches title and artist

### SearchableItem Enum

```rust
pub enum SearchableItem {
    Folder(FavoriteFolder),
    Resource(FavoriteResource),
    Episode(VideoPage),
    WatchLater(WatchLaterItem),
    History(HistoryItem),
    PlaylistItem(PlaylistItem),
}
```

## Error Handling

### Error Scenarios
- **API failures:** Show error in status bar, keep current list
- **Network timeout:** Show "Search timeout", allow retry
- **Empty search query:** Show all items (no filtering)

### Edge Cases
- **Empty results:** Display "No results found for '{query}'"
- **Search during loading:** Disable search while loading
- **Tab switching:** Clear search state when switching tabs
- **Case sensitivity:** Case-insensitive search (lowercase comparison)
- **Whitespace:** Trim query, ignore leading/trailing spaces
- **Large lists:** Limit displayed results to 100 items

### Performance Optimizations
- Use iterators with `filter()` for local searches
- Debounce API calls (300ms)
- Limit API results to 100 items
- Cache last search query to avoid redundant calls

## Testing Strategy

### Unit Tests
- `Searchable` trait implementations
- Query matching (case-insensitive, partial matches)
- Empty query handling
- `SearchState` transitions

### Integration Tests
- Search activation with "/" key
- Query input and filtering
- API search calls with mocks
- Exit and restoration of original list

### Manual Testing Checklist
1. Press "/" in each tab
2. Type query and verify filtering
3. Navigate results with j/k
4. Press Esc and verify restoration
5. Search with no matches
6. Search with special characters
7. Search with long query (100+ chars)
8. Switch tabs during search
9. Search on empty lists
10. Test network errors

## Implementation Plan

### Files to Create
- `src/screens/search.rs` - Search state management
- `src/ui/widgets/search_bar.rs` - Search bar widget

### Files to Modify
- `src/screens/library.rs` - Add search mode integration
- `src/api/client.rs` - Add search API methods
- `src/app.rs` - Handle "/" key binding
- `src/ui/widgets/mod.rs` - Export search bar widget

### Implementation Order
1. Create `SearchState` and `Searchable` trait
2. Implement `Searchable` for all item types
3. Create search bar widget
4. Add search API methods to `BilibiliClient`
5. Integrate search mode into `LibraryScreen`
6. Add "/" key binding to `App`
7. Write tests
8. Manual testing and refinement

## Success Criteria

- [ ] "/" key activates search in all target pages
- [ ] Search bar overlay appears correctly
- [ ] API searches work for Favorites, Watch Later, History
- [ ] Local filtering works for Playing List and Episodes
- [ ] Esc restores original list
- [ ] j/k navigation works on filtered results
- [ ] Empty results show appropriate message
- [ ] Case-insensitive search works
- [ ] Debouncing prevents excessive API calls
- [ ] All tests pass
