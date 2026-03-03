# MPRIS Integration Design

**Date:** 2026-03-02  
**Status:** Approved  
**Author:** Claude (with user input)

## Overview

Add MPRIS (Media Player Remote Interfacing Specification) support to biu-tui, enabling desktop environments to display currently playing track information and allowing users to control playback through system media controls.

**Scope:** Full MPRIS support including metadata, playback controls (play/pause/stop/next/prev), seeking, and volume control. No album art.

## Requirements

### Functional Requirements
- Display track metadata (title, artist, duration) in desktop media controls
- Support playback state synchronization (playing/paused/stopped)
- Handle incoming MPRIS commands: play, pause, stop, next, previous, seek, volume
- Support position updates for progress bar display
- Work on Linux desktop environments (GNOME, KDE, etc.)

### Non-Functional Requirements
- MPRIS failures must not crash or prevent app startup
- Non-blocking operation - no UI freezes from D-Bus communication
- Graceful degradation on headless systems without D-Bus

## Architecture

### Module Structure

**New file:** `src/mpris.rs`

```
src/
├── mpris.rs          # New module for MPRIS integration
├── lib.rs            # Export mpris module
├── app.rs            # Integration with MprisManager
└── ...
```

### Component Design

```rust
pub enum MprisCommand {
    Play,
    Pause,
    Stop,
    Next,
    Previous,
    Seek(Duration),
    SetVolume(f32),
}

pub struct MprisManager {
    player: mpris::Player,
}

impl MprisManager {
    pub fn new() -> Result<Self>;
    pub fn set_track(&self, item: &PlaylistItem);
    pub fn set_state(&self, state: PlayerState);
    pub fn set_position(&self, position: Duration);
    pub fn set_volume(&self, volume: f32);
    pub fn poll_commands(&mut self) -> Vec<MprisCommand>;
}
```

### Integration Points

1. **Initialization** - MprisManager created in `App::init_app_state()`
2. **State Updates** - App notifies MprisManager on:
   - Track changes (`play_playlist_item`)
   - Playback state changes (`toggle_playback`, `pause`, `resume`, `stop`)
   - Position changes (polled in main loop)
   - Volume changes (`set_volume`)
3. **Command Handling** - MprisManager polls for commands in main event loop

### Data Flow

```
User Action → App → AudioPlayer
                    ↓
              MprisManager → D-Bus → Desktop Environment

Desktop Environment → D-Bus → MprisManager.poll_commands()
                                ↓
                              App.handle_mpris_command()
```

## Implementation Details

### Dependencies

Add to `Cargo.toml`:
```toml
mpris = "2.0"
```

### App Struct Changes

```rust
pub struct App {
    // ... existing fields ...
    mpris: Option<MprisManager>,
}
```

### Initialization

```rust
// In App::init_app_state()
let mpris = MprisManager::new()
    .map_err(|e| eprintln!("MPRIS initialization failed: {}", e))
    .ok();
```

### State Update Call Sites

1. **Track change** in `play_playlist_item()`:
   ```rust
   if let Some(mpris) = &self.mpris {
       mpris.set_track(item);
       mpris.set_state(PlayerState::Playing);
   }
   ```

2. **Playback state** in `toggle_playback()`, `pause_playback()`, etc.

3. **Position updates** in main event loop:
   ```rust
   if let (Some(player), Some(mpris)) = (&self.player, &self.mpris) {
       mpris.set_position(player.position());
   }
   ```

4. **Command polling** in main event loop:
   ```rust
   if let Some(mpris) = &mut self.mpris {
       for cmd in mpris.poll_commands() {
           self.handle_mpris_command(cmd)?;
       }
   }
   ```

## Error Handling

### Initialization Failure
- MPRIS initialization failure does NOT crash the app
- App stores `Option<MprisManager>`
- If MPRIS unavailable (headless, no D-Bus), app continues normally

### Runtime Errors
- Individual MPRIS operations log errors with `eprintln!` but don't propagate
- MPRIS is a "nice to have" feature, not critical path
- `poll_commands()` returns empty Vec on D-Bus errors

### Edge Cases

1. **No Track Playing** - `set_track()` with None clears MPRIS metadata
2. **Seeking Beyond Duration** - Validate and clamp seek positions
3. **Rapid State Changes** - Queue multiple commands in poll_commands() Vec
4. **App Startup Before D-Bus Ready** - Retry connection 3 times with 100ms delays
5. **Headless Environment** - Fail gracefully without D-Bus session bus

## Testing Strategy

### Unit Tests
- MprisCommand enum construction and matching
- PlayerState to MPRIS PlaybackStatus mapping
- PlaylistItem to MPRIS metadata conversion

### Integration Tests
- Mock MprisManager for testing App integration without real D-Bus
- Command flow from MPRIS to App methods

### Manual Testing
- GNOME Shell media indicator
- KDE Plasma media widget
- System media controls
- Media keys (play/pause, next, previous)
- `playerctl` CLI tool verification:
  ```bash
  playerctl -p biu-tui metadata
  playerctl -p biu-tui play
  playerctl -p biu-tui next
  ```
- Multiple instance conflict testing

### No Automated D-Bus Tests
- Full D-Bus integration requires running session bus (too complex for CI)
- Rely on manual testing and unit tests of logic

## Dependencies

- **mpris crate v2.0** - Well-maintained Rust MPRIS library
- Requires D-Bus session bus (standard on Linux desktop environments)

## Out of Scope

- Album art display (not included in initial implementation)
- Windows/macOS support (MPRIS is Linux-specific)
- MPRIS TrackList interface (only current track)
- MPRIS Playlists interface

## Future Enhancements

- Album art from Bilibili thumbnails
- Rating support
- Shuffle/repeat mode indicators
