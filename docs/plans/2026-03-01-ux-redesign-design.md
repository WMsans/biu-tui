# UX Redesign Design

## Summary

Redesign the biu-tui interface to add:
1. Breadcrumb navigation bar showing folder hierarchy
2. Episode list support for multi-part videos (分p视频)
3. ESC key for hierarchical navigation
4. Progress bar with time display below now-playing

## Overall Layout

```
┌──────────────────────────────────────────────────────┐
│ [Favorites]  [Watch Later]  [History]                │  ← Tab Bar
├──────────────────────────────────────────────────────┤
│ Favorites > My Music Folder                          │  ← Breadcrumb Bar (NEW)
├──────────────────────────────────────────────────────┤
│                                                      │
│   ▶ Video Title 1                           4:32     │
│   ▶ Video Title 2 (P1~3)                    12:45    │  ← List Area
│   ▶ Video Title 3                           3:21     │
│                                                      │
├──────────────────────────────────────────────────────┤
│ ♫ Now Playing: Video Title - Uploader Name           │  ← Now Playing Bar
│ ━━━━━━━━━━━╾─────────────────  02:34 / 04:12        │  ← Progress Bar (NEW)
├──────────────────────────────────────────────────────┤
│ [j/k] Nav [Enter] Select [Esc] Back [Space] Pause [q] Quit │
└──────────────────────────────────────────────────────┘
```

## Navigation Hierarchy

### Levels

```
Level 0: Tab Selection (Favorites / Watch Later / History)
Level 1: Folder List
Level 2: Video List (inside a folder)
Level 3: Episode List (if video has multiple parts)
```

### Flow

```
Favorites Tab
    │
    ├─[Enter]──→ Folder List
    │                │
    │                ├─[Enter]──→ Video List
    │                │                │
    │                │                ├─[Enter]──→ Episode List (if multi-part)
    │                │                │                │
    │                │                │                └─[Enter]──→ Play episode
    │                │                │
    │                │                └─[Enter]──→ Play video (if single-part)
    │                │
    │                └─[Esc]──→ Back to Folder List
    │
    └─[Esc]──→ (stay at root, 'q' to quit)
```

### Breadcrumb Updates

- `Favorites` — at folder list
- `Favorites > My Music` — inside a folder
- `Favorites > My Music > Video Title` — at episode list

### Key Bindings

| Key | Action |
|-----|--------|
| `Enter` | Navigate into / play |
| `Esc` | Go back one level |
| `q` | Quit app |
| `Space` | Play/Pause |

## Progress Bar

### Design

```
━━━━━━━━━━╾─────────────────  02:34 / 04:12
```

- Filled portion: `━` characters
- Current position marker: `╾`
- Unfilled portion: `─` characters
- Time format: `MM:SS / MM:SS` (current / total)

### State Indicators

| State | Now Playing Prefix | Progress Display |
|-------|-------------------|------------------|
| Playing | `♫` | Shows progress |
| Paused | `⏸` | Shows progress |
| Stopped | `○` | `━━────────────  --:-- / --:--` |

## Episode List (分p视频)

### Video List Entry

```
▶ Video Title (P1~3)                              12:45
```

- Show `(P1~N)` indicator when video has multiple parts
- Duration shows total duration

### Episode List View

```
Breadcrumb: Favorites > My Music > Video Title

┌──────────────────────────────────────────────────────┐
│   P1: First Episode Title                        3:45│
│   P2: Second Episode Title                       4:12│
│   P3: Third Episode Title                        4:48│
└──────────────────────────────────────────────────────┘
```

### Behavior

- Single-part video: `Enter` plays immediately
- Multi-part video: `Enter` shows episode list first
- At episode list: `Enter` plays selected episode

## Implementation

### Files to Modify

1. **`src/app.rs`** - Add navigation state enum and breadcrumb tracking
2. **`src/screens/library.rs`** - Add breadcrumb and progress bar rendering
3. **`src/audio/player.rs`** - Ensure position/duration are accessible

### New State Types

```rust
enum NavigationLevel {
    Folders,
    Videos { folder_id: u64, folder_title: String },
    Episodes { folder_id: u64, bvid: String, video_title: String },
}
```

### State Tracking

- `breadcrumb: Vec<String>` — path segments
- `current_level: NavigationLevel` — where we are
- `episodes: Option<Vec<VideoPage>>` — loaded episode list

### Key Handling

- `Esc`: Match on `current_level`, go to parent level
- Remove `Esc` → quit behavior, only `q` quits
