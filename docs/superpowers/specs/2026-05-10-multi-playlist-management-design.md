# Design: Multiple Playlist Management

**Date:** 2026-05-10
**Status:** Approved

## Overview

Add support for creating and managing multiple named playlists, stored locally. Rename the current "Playing List" tab to "Playing Now" (the transient playback queue). Add a new "Playlists" tab for managing persistent named playlists. Clean up breadcrumb display: only Favorites shows folder navigation breadcrumbs; Playlists shows the current playlist name; other tabs show nothing.

## Data Model

### PlaylistItem (unchanged)

```rust
pub struct PlaylistItem {
    pub bvid: String,
    pub cid: u64,
    pub title: String,
    pub artist: String,
    pub duration: u32,
}
```

### Playlist

```rust
pub struct Playlist {
    pub name: String,
    pub items: Vec<PlaylistItem>,
}
```

## Storage

- **Directory:** `~/.config/biu-tui/playlists/`
- **One file per playlist:** `{sanitized_name}.json` containing the `Playlist` struct
- **Index file:** `playlists/.index.json` — a sorted `Vec<String>` of playlist names
- **Sanitization:** filesystem-unfriendly chars (`/`, `\`, `:`, etc.) replaced with `_` for filenames only; display name keeps original characters
- **Atomic writes:** write to `.tmp`, then rename (same pattern as existing PlayingListManager)

## Module Structure

### New: `src/playlists/mod.rs`

`PlaylistManager` with:
- `new() -> Result<Self>` — creates `playlists/` directory, loads index
- `list_names() -> Vec<String>` — all playlist names from index
- `load(name: &str) -> Result<Playlist>`
- `save(playlist: &Playlist) -> Result<()>`
- `create(name: &str) -> Result<()>` — validates name, creates empty playlist, adds to index
- `delete(name: &str) -> Result<()>` — removes file and index entry
- `rename(old: &str, new: &str) -> Result<()>` — validates new name, renames file, updates index
- `add_item(playlist_name: &str, item: PlaylistItem) -> Result<()>`
- `remove_item(playlist_name: &str, index: usize) -> Result<()>`
- `reorder(playlist_name: &str, from: usize, to: usize) -> Result<()>`
- `get_items(playlist_name: &str) -> Result<Vec<PlaylistItem>>`

Shared via `Arc<Mutex<PlaylistManager>>` in `App`.

### Preserved: `src/playing_list/mod.rs`

Unchanged. This remains the Playing Now queue, persisted to `playing_list.json`.

### Changes to existing files

| File | Change |
|---|---|
| `src/lib.rs` | Add `pub mod playlists;` |
| `src/storage/mod.rs` | Add `pub mod playlists;`, re-export `PlaylistManager` |
| `src/screens/library.rs` | Add `Playlists` variant to `LibraryTab`; add playlist UI state fields; conditional breadcrumb; handle Playlists tab keybindings; implement unified "add to" prompt widget |
| `src/app.rs` | Add `playlist_manager` field; pass to library; rename `PlayingList` to `PlayingNow` |

## UI Layout

### Library Tabs (5 tabs, cycled with `Tab`)

```
[Favorites] [Watch Later] [History] [Playing Now] [Playlists]
```

### Breadcrumb (conditional per tab)

| Tab | Breadcrumb |
|---|---|
| Favorites | `Favorites` or `Favorites > {folder}` or `Favorites > {folder} > {video}` |
| Watch Later | *(hidden)* |
| History | *(hidden)* |
| Playing Now | *(hidden)* |
| Playlists | `Playlists` or `Playlists > {playlist_name}` |

When no breadcrumb is shown, the layout reclaims that row for content.

### Playlists Tab — Two Navigation Levels

**Level 1: Playlist list**
- Shows playlist names with track counts: `Workout (12)`, `Study (8)`
- `Enter` — opens that playlist's contents
- `n` — create new playlist (inline name prompt)
- `d` — delete selected playlist (confirmation toast)
- `r` — rename selected playlist (inline prompt)

**Level 2: Playlist contents**
- Shows tracks in the selected playlist (same layout as Playing Now)
- `Enter` — play selected track (loads entire playlist into Playing Now, starts at selection)
- `d` — remove track from playlist
- `Ctrl+↑` / `Ctrl+↓` — reorder selected track up/down
- `Esc` / `Backspace` — back to playlist list

## Unified "Add to" Prompt

Triggered by `a` from Favorites, Watch Later, or History. Triggered by `A` (add all) from those same tabs.

- A small popup appears: `"Add '{title}' to:"` (or `"Add {N} items to:"` for add-all)
- Target list shows: `Playing Now` (always first), then each playlist alphabetically
- Navigate with `j`/`k`, confirm with `Enter`, cancel with `Esc`
- On confirm: item(s) added, brief toast `"Added to {destination}"`
- Underlying list selection is preserved (non-destructive popup)

`a` and `A` do nothing when already on the Playing Now or Playlists tabs.

## Playlist ↔ Playing Now Interaction

- Playing a playlist (Enter on a track in level 2) **loads all tracks into Playing Now** (replacing current), then starts playback at the selected track
- Individual ad-hoc additions (via `a` from other tabs) go wherever the user picks in the prompt — either Playing Now or a playlist
- After a playlist is loaded into Playing Now, subsequent individual additions are appended

## Error Handling & Edge Cases

### Name validation
- Non-empty, trimmed, max 64 characters
- Duplicate names (case-insensitive) rejected with toast error

### Corrupted/missing files
- Missing `.index.json`: rebuild from `playlists/*.json` files
- Corrupted playlist JSON: skip, show `"(corrupted)"` marker, log warning
- Missing playlist file (in index but not on disk): auto-remove from index

### Empty playlists
- Can be created with `(0)` count
- Selecting empty playlist and pressing Enter shows `"No tracks in playlist"` toast
- Can still be deleted or renamed

## Testing

- Unit tests for `PlaylistManager`: create, delete, rename, add, remove, reorder, persistence round-trip
- Tests for name sanitization and index auto-rebuild
- Integration test: create → add items → play → verify Playing Now loaded
