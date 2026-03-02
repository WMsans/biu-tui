# Playing List Feature Design

**Date:** 2026-03-01  
**Status:** Approved  

## Overview

This document describes the design for a persistent playing list feature that allows users to queue songs, manage the queue, and control playback behavior.

## Requirements

### Functional Requirements

1. **Loop Mode Changes**
   - Remove `LoopFolder` mode
   - Keep `LoopOne` mode (loops current song)
   - Keep `NoLoop` mode (stops at end)
   - Add `LoopList` mode (loops entire playing list)

2. **Playing List Management**
   - Display playing list as a 4th tab in Library screen
   - Add single song to playing list
   - Add all songs from current folder to playing list
   - Remove songs from playing list
   - Jump to any song in playing list (play immediately)
   - Playing list persists across app restarts

3. **Playback Behavior**
   - When removing currently playing song: skip to next song
   - When adding song: append to end of queue
   - When jumping to song: play immediately
   - When playback completes: follow loop mode behavior

### Non-Functional Requirements

- Playing list saves to disk automatically
- UI remains responsive during save operations
- Graceful error handling (no crashes on I/O errors)
- Clear user feedback for all operations

## Architecture

### Component Structure

```
┌─────────────────────────────────────────┐
│           LibraryScreen (UI)            │
│  - Displays Playing List tab            │
│  - Handles user input                   │
│  - Calls PlayingListManager methods     │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│       PlayingListManager (Logic)        │
│  - Manages queue state                  │
│  - Handles add/remove/jump operations   │
│  - Tracks current position              │
│  - Coordinates with storage             │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│    PlayingListStorage (Persistence)     │
│  - Saves to playing_list.json           │
│  - Loads on app startup                 │
│  - Stores basic song info               │
└─────────────────────────────────────────┘
```

### Module Organization

- `src/playing_list/mod.rs` - Manager and types
- `src/storage/playing_list.rs` - Persistence layer
- Integration in `src/app.rs` and `src/screens/library.rs`

## Data Structures

### PlaylistItem

Represents a song in the playing list with minimal required information:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub bvid: String,
    pub cid: u64,
    pub title: String,
    pub artist: String,
    pub duration: u32,
}
```

**Rationale:** Basic info allows display without API calls, keeping UI fast. Can fetch full metadata on-demand if needed.

### PlayingListManager

Manages the queue state and operations:

```rust
pub struct PlayingListManager {
    items: Vec<PlaylistItem>,
    current_index: Option<usize>,
    storage_path: PathBuf,
}

impl PlayingListManager {
    pub fn new() -> Result<Self>;
    pub fn add(&mut self, item: PlaylistItem);
    pub fn add_all(&mut self, items: Vec<PlaylistItem>);
    pub fn remove(&mut self, index: usize) -> Option<PlaylistItem>;
    pub fn jump_to(&mut self, index: usize);
    pub fn current(&self) -> Option<&PlaylistItem>;
    pub fn next(&mut self) -> Option<&PlaylistItem>;
    pub fn clear(&mut self);
}
```

### LoopMode Update

Modified enum with new mode:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    LoopOne,      // Keep: loops current song
    NoLoop,       // Keep: stops at end
    LoopList,     // NEW: loops entire playing list
    // LoopFolder removed
}
```

## User Interface

### Tab Navigation

- Add `LibraryTab::PlayingList` as 4th tab
- Tab order: Favorites → Watch Later → History → Playing List
- Show count badge in tab: "Playing List (3)"

### Playing List Tab Layout

```
┌─────────────────────────────────────────┐
│ [Favorites] [Watch Later] [History] [Playing List (3)]
├─────────────────────────────────────────┤
│ Playing List                            │
├─────────────────────────────────────────┤
│ ♫ Song Title 1 - Artist A      3:45    │
│   Song Title 2 - Artist B      4:20    │
│   Song Title 3 - Artist C      5:10    │
├─────────────────────────────────────────┤
│ ♫ Now Playing: Song Title 1 - Artist A │
├─────────────────────────────────────────┤
│ [j/k] Navigate  [Enter] Jump  [d] Remove
└─────────────────────────────────────────┘
```

### Key Bindings

**In Playing List Tab:**
- `j/k` or `↑/↓` - Navigate list
- `Enter` - Jump to song (play immediately)
- `d` - Remove selected song
- `Esc` - No action (already on top level)

**In Other Tabs (Favorites/Watch Later/History):**
- `a` - Add selected song to playing list
- `A` (Shift+a) - Add all songs in current folder to playing list

### Visual Indicators

- Current song marked with `♫` symbol
- Other songs show empty space
- Empty list shows message: "No songs in playing list. Press 'a' to add songs."

## Data Flow

### Adding a Song

```
User presses 'a'
  → LibraryScreen::handle_add_to_playing_list()
    → PlayingListManager::add(item)
      → PlayingListStorage::save(items)
  → Show confirmation message
```

### Adding All Songs

```
User presses 'A'
  → LibraryScreen::handle_add_all_to_playing_list()
    → Collect all songs from current folder/tab
    → PlayingListManager::add_all(items)
      → PlayingListStorage::save(items)
  → Show confirmation with count
```

### Jumping to a Song

```
User presses Enter on song
  → LibraryScreen::handle_jump_to_song(index)
    → PlayingListManager::jump_to(index)
      → Returns PlaylistItem
    → Play audio stream for item
    → Update now_playing
```

### Removing a Song

```
User presses 'd'
  → LibraryScreen::handle_remove_song(index)
    → Check if removing current song
      → If yes: PlayingListManager::next() → play next
    → PlayingListManager::remove(index)
      → PlayingListStorage::save(items)
```

### Playback Completion

```
Song ends
  → App::poll_playback_completion()
    → Check loop_mode
      → LoopOne: replay current
      → NoLoop: PlayingListManager::next() → play or stop if None
      → LoopList: PlayingListManager::next() → play (wraps to start)
```

## Persistence

### Storage Location

- File: `~/.config/biu-tui/playing_list.json`
- Same directory as `settings.json`

### File Format

```json
{
  "items": [
    {
      "bvid": "BV1xx411c7mD",
      "cid": 12345678,
      "title": "Song Title",
      "artist": "Artist Name",
      "duration": 225
    }
  ],
  "current_index": 0
}
```

### Save Strategy

- **When:** Automatic on every modification (add, remove, jump)
- **How:** Atomic write (write to temp file, then rename)
- **Error handling:** Log errors, continue in-memory (no crashes)

### Load Strategy

- **When:** On app startup in `App::new()`
- **How:** Read file, deserialize JSON
- **Error handling:** If missing/corrupted, start with empty list

### Migration

- No migration needed (new feature)
- If file doesn't exist, start with empty list

## Error Handling

### Storage Errors

| Error | Action |
|-------|--------|
| Failed to load | Log warning, start with empty list |
| Failed to save | Log error, continue in-memory |
| Corrupted file | Log error, delete file, start fresh |

### Playback Errors

| Error | Action |
|-------|--------|
| Failed to get audio stream | Skip to next song, show error message |
| Song removed during playback | Skip to next automatically |

### UI Edge Cases

| Case | Behavior |
|------|----------|
| Empty playing list + jump | No-op, show "Playing list is empty" |
| Empty playing list + remove | No-op |
| Index out of bounds | Defensive: clamp indices, return None |
| API error when adding | Show error, don't add to list |

### Error Display

- Use status bar or temporary message
- Auto-clear after 3 seconds
- Example: "Failed to add song: Network error"

## Testing Strategy

### Unit Tests

**PlayingListManager:**
- `test_add_item` - Add single item
- `test_add_multiple_items` - Add multiple items
- `test_remove_item` - Remove item by index
- `test_remove_current_item_advances_to_next` - Edge case
- `test_jump_to_song` - Jump changes current index
- `test_next_wraps_in_loop_list_mode` - Wrap behavior
- `test_next_stops_in_no_loop_mode` - Stop behavior
- `test_persistence_save_and_load` - Round-trip test
- `test_empty_list_operations` - Edge cases

**LoopMode:**
- `test_loop_mode_next_sequence` - Verify cycle
- `test_loop_mode_prev_sequence` - Verify reverse cycle
- `test_loop_folder_removed` - Ensure removed

### Integration Tests

- Full flow: add → play → skip → remove
- Persistence: add → restart → verify restored
- Playback completion with different loop modes

### Manual Testing Checklist

1. Add single song to playing list
2. Add all songs from folder
3. Jump to different songs in list
4. Remove current song (should skip to next)
5. Remove non-current song
6. Verify loop modes work correctly
7. Restart app and verify persistence
8. Test with empty playing list
9. Test with network errors
10. Test keyboard shortcuts (a, A, d, Enter)

## Implementation Plan

### Files to Create

- `src/playing_list/mod.rs` - PlayingListManager and PlaylistItem
- `src/storage/playing_list.rs` - Persistence layer

### Files to Modify

- `src/storage/settings.rs` - Update LoopMode enum
- `src/screens/library.rs` - Add PlayingList tab and key handlers
- `src/app.rs` - Initialize PlayingListManager, update playback logic
- `src/storage/mod.rs` - Export playing_list module

### Implementation Order

1. Update LoopMode enum in `settings.rs`
2. Create PlaylistItem and PlayingListManager structs
3. Implement persistence (save/load)
4. Add PlayingListManager to App
5. Add Playing List tab to LibraryScreen
6. Implement key handlers (add, add all, jump, remove)
7. Update playback completion logic in App
8. Add unit tests
9. Manual testing

### Estimated Effort

- **Complexity:** Medium
- **Lines of Code:** ~300-400 new lines
- **Time:** 1-2 days for implementation and testing

## Future Enhancements

These are out of scope for the initial implementation but could be added later:

- Reorder songs in playing list (drag and drop or key bindings)
- Save multiple named playlists
- Import/export playlists
- Shuffle mode
- Search/filter within playing list
- Show album art or thumbnails

## References

- Similar feature in music players: Spotify queue, Apple Music Up Next
- Rust patterns: Arc<Mutex<T>> for shared state
- Ratatui patterns: Stateful widgets, event handling
