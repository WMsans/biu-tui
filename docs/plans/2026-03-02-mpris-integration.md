# MPRIS Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add MPRIS support to biu-tui so the currently playing track appears in desktop media controls and users can control playback through system media interfaces.

**Architecture:** Create a new `src/mpris.rs` module that wraps the `mpris` crate. The App struct holds an `Option<MprisManager>` and notifies it of state changes. MprisManager polls for incoming commands and returns them to the App for execution.

**Tech Stack:** Rust, mpris crate v2.0, D-Bus

---

## Task 1: Add mpris dependency

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add mpris dependency**

Add to `Cargo.toml` in the `[dependencies]` section:

```toml
mpris = "2.0"
```

**Step 2: Verify dependency resolves**

Run: `cargo check`
Expected: Success, downloads mpris crate and its dependencies

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat: add mpris dependency"
```

---

## Task 2: Create MprisManager struct and enum

**Files:**
- Create: `src/mpris.rs`

**Step 1: Write the MprisCommand enum and MprisManager struct**

Create `src/mpris.rs`:

```rust
use anyhow::{Context, Result};
use mpris::{Metadata, PlaybackStatus, Player, PlayerBuilder};
use std::time::Duration;

use crate::audio::PlayerState;
use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone, PartialEq)]
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
    player: Player,
}

impl MprisManager {
    pub fn new() -> Result<Self> {
        let player = PlayerBuilder::new("biu-tui", "biu-tui")
            .identity("Biu TUI")
            .desktop_entry("biu-tui")
            .supports_track_lists(false)
            .build()
            .context("Failed to create MPRIS player")?;

        Ok(Self { player })
    }

    pub fn set_track(&self, item: &PlaylistItem) {
        let mut metadata = Metadata::new();
        metadata.artist = Some(vec![item.artist.clone()]);
        metadata.title = Some(item.title.clone());
        metadata.length = Some(Duration::from_secs(item.duration as u64));

        if let Err(e) = self.player.set_metadata(metadata) {
            eprintln!("Failed to set MPRIS metadata: {}", e);
        }
    }

    pub fn set_state(&self, state: PlayerState) {
        let status = match state {
            PlayerState::Playing => PlaybackStatus::Playing,
            PlayerState::Paused => PlaybackStatus::Paused,
            PlayerState::Stopped => PlaybackStatus::Stopped,
        };

        if let Err(e) = self.player.set_playback_status(status) {
            eprintln!("Failed to set MPRIS playback status: {}", e);
        }
    }

    pub fn set_position(&self, position: Duration) {
        if let Err(e) = self.player.set_position(position) {
            eprintln!("Failed to set MPRIS position: {}", e);
        }
    }

    pub fn set_volume(&self, volume: f32) {
        if let Err(e) = self.player.set_volume(volume as f64) {
            eprintln!("Failed to set MPRIS volume: {}", e);
        }
    }

    pub fn poll_commands(&mut self) -> Vec<MprisCommand> {
        let mut commands = Vec::new();

        if let Ok(events) = self.player.events() {
            for event in events {
                match event {
                    Ok(mpris::Event::Play) => commands.push(MprisCommand::Play),
                    Ok(mpris::Event::Pause) => commands.push(MprisCommand::Pause),
                    Ok(mpris::Event::Stop) => commands.push(MprisCommand::Stop),
                    Ok(mpris::Event::Next) => commands.push(MprisCommand::Next),
                    Ok(mpris::Event::Previous) => commands.push(MprisCommand::Previous),
                    Ok(mpris::Event::Seek { position }) => {
                        commands.push(MprisCommand::Seek(position))
                    }
                    Ok(mpris::Event::Volume { volume }) => {
                        commands.push(MprisCommand::SetVolume(volume as f32))
                    }
                    Err(e) => eprintln!("MPRIS event error: {}", e),
                    _ => {}
                }
            }
        }

        commands
    }
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/mpris.rs
git commit -m "feat: create MprisManager with basic API"
```

---

## Task 3: Export mpris module

**Files:**
- Modify: `src/lib.rs`

**Step 1: Add mpris module to lib.rs**

Add to `src/lib.rs` in the module declarations section:

```rust
pub mod mpris;
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/lib.rs
git commit -m "feat: export mpris module"
```

---

## Task 4: Add mpris field to App struct

**Files:**
- Modify: `src/app.rs`

**Step 1: Import MprisManager and MprisCommand**

Add to imports at top of `src/app.rs`:

```rust
use crate::mpris::{MprisCommand, MprisManager};
```

**Step 2: Add mpris field to App struct**

Add field to `App` struct (around line 38):

```rust
pub struct App {
    // ... existing fields ...
    mpris: Option<MprisManager>,
}
```

**Step 3: Initialize mpris in init_app_state**

In `init_app_state()` function, add before the `Ok(Self {` line (around line 98):

```rust
let mpris = MprisManager::new()
    .map_err(|e| eprintln!("MPRIS initialization failed: {}", e))
    .ok();
```

**Step 4: Add mpris to App initialization**

Add to the `Ok(Self {` initialization (around line 111):

```rust
Ok(Self {
    // ... existing fields ...
    mpris,
})
```

**Step 5: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 6: Commit**

```bash
git add src/app.rs
git commit -m "feat: add mpris field to App struct"
```

---

## Task 5: Update MPRIS on track change

**Files:**
- Modify: `src/app.rs`

**Step 1: Notify MPRIS when track starts**

In `play_playlist_item()` method, add after the `p.play()` call (around line 411):

```rust
pub fn play_playlist_item(&mut self, item: &PlaylistItem) -> Result<()> {
    // ... existing code ...
    
    if let Some(p) = &mut self.player {
        p.play(&audio_stream.url)?;
        
        // Notify MPRIS
        if let Some(mpris) = &self.mpris {
            mpris.set_track(item);
            mpris.set_state(PlayerState::Playing);
        }
        
        if let Screen::Library(library) = &mut self.screen {
            library.now_playing = Some((item.title.clone(), item.artist.clone()));
        }
    }

    self.apply_volume();
    Ok(())
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: update MPRIS metadata on track change"
```

---

## Task 6: Update MPRIS on playback state changes

**Files:**
- Modify: `src/app.rs`

**Step 1: Update MPRIS in toggle_playback**

In `toggle_playback()` method, add MPRIS state updates (around line 421):

```rust
fn toggle_playback(&mut self) -> Result<()> {
    if let Some(player) = &self.player {
        match player.state() {
            PlayerState::Playing => {
                player.pause();
                if let Some(mpris) = &self.mpris {
                    mpris.set_state(PlayerState::Paused);
                }
            }
            PlayerState::Paused => {
                player.resume();
                if let Some(mpris) = &self.mpris {
                    mpris.set_state(PlayerState::Playing);
                }
            }
            PlayerState::Stopped => {
                self.start_playback_if_available()?;
            }
        }
    } else {
        self.start_playback_if_available()?;
    }
    Ok(())
}
```

**Step 2: Update MPRIS in stop**

Find the `stop()` method or create one if it doesn't exist. If `stop()` exists in App, add MPRIS notification. If not, add it after `toggle_playback()`:

```rust
fn stop(&mut self) -> Result<()> {
    if let Some(player) = &mut self.player {
        player.stop();
        if let Some(mpris) = &self.mpris {
            mpris.set_state(PlayerState::Stopped);
        }
    }
    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: update MPRIS state on playback changes"
```

---

## Task 7: Poll MPRIS commands in main loop

**Files:**
- Modify: `src/app.rs`

**Step 1: Add handle_mpris_command method**

Add new method after `toggle_playback()` (around line 438):

```rust
fn handle_mpris_command(&mut self, cmd: MprisCommand) -> Result<()> {
    match cmd {
        MprisCommand::Play => {
            if let Some(player) = &self.player {
                if player.state() == PlayerState::Paused {
                    player.resume();
                    if let Some(mpris) = &self.mpris {
                        mpris.set_state(PlayerState::Playing);
                    }
                }
            }
        }
        MprisCommand::Pause => {
            if let Some(player) = &self.player {
                if player.state() == PlayerState::Playing {
                    player.pause();
                    if let Some(mpris) = &self.mpris {
                        mpris.set_state(PlayerState::Paused);
                    }
                }
            }
        }
        MprisCommand::Stop => {
            self.stop()?;
        }
        MprisCommand::Next => {
            let next_item = self.playing_list.lock().advance_to_next().cloned();
            if let Some(item) = next_item {
                self.play_playlist_item(&item)?;
            }
        }
        MprisCommand::Previous => {
            let prev_item = self.playing_list.lock().advance_to_previous().cloned();
            if let Some(item) = prev_item {
                self.play_playlist_item(&item)?;
            }
        }
        MprisCommand::Seek(_position) => {
            if let Some(player) = &self.player {
                player.seek(_position);
                if let Some(mpris) = &self.mpris {
                    mpris.set_position(_position);
                }
            }
        }
        MprisCommand::SetVolume(volume) => {
            self.settings.volume = (volume * 100.0) as u32;
            self.apply_volume();
            if let Some(mpris) = &self.mpris {
                mpris.set_volume(volume);
            }
        }
    }
    Ok(())
}
```

**Step 2: Poll MPRIS commands in run loop**

In the `run()` method, add MPRIS polling after event handling (around line 166):

```rust
pub fn run(&mut self) -> Result<()> {
    while self.running {
        // ... existing draw code ...

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('c')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    self.running = false;
                    continue;
                }
                self.handle_key(key.code, key.modifiers)?;
            }
        }

        // Poll MPRIS commands
        if let Some(mpris) = &mut self.mpris {
            for cmd in mpris.poll_commands() {
                if let Err(e) = self.handle_mpris_command(cmd) {
                    eprintln!("Failed to handle MPRIS command: {}", e);
                }
            }
        }

        self.poll_qr_login()?;
        self.poll_playback_completion()?;
    }

    self.cleanup()?;
    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: poll and handle MPRIS commands"
```

---

## Task 8: Update MPRIS position in main loop

**Files:**
- Modify: `src/app.rs`

**Step 1: Add position update to run loop**

In the `run()` method, add position update before polling MPRIS commands (around line 165):

```rust
// Update MPRIS position
if let (Some(player), Some(mpris)) = (&self.player, &self.mpris) {
    mpris.set_position(player.position());
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/app.rs
git commit -m "feat: update MPRIS position in main loop"
```

---

## Task 9: Add advance_to_previous to PlayingListManager

**Files:**
- Modify: `src/playing_list/mod.rs`

**Step 1: Add advance_to_previous method**

In `src/playing_list/mod.rs`, add method after `advance_to_next()` (check where it is first):

```rust
pub fn advance_to_previous(&mut self) -> Option<&PlaylistItem> {
    if self.items.is_empty() {
        return None;
    }

    let new_index = match self.current_index {
        Some(idx) if idx > 0 => idx - 1,
        Some(_) => self.items.len() - 1,  // Wrap to last
        None => 0,
    };

    self.jump_to(new_index);
    self.current()
}
```

**Step 2: Verify compilation**

Run: `cargo check`
Expected: Success

**Step 3: Commit**

```bash
git add src/playing_list/mod.rs
git commit -m "feat: add advance_to_previous to PlayingListManager"
```

---

## Task 10: Run clippy and fix warnings

**Files:**
- Various

**Step 1: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: May have warnings

**Step 2: Fix any warnings**

Address any clippy warnings reported.

**Step 3: Re-run clippy**

Run: `cargo clippy -- -D warnings`
Expected: Success with no warnings

**Step 4: Commit fixes**

```bash
git add -A
git commit -m "fix: resolve clippy warnings"
```

---

## Task 11: Test MPRIS integration

**Files:**
- N/A

**Step 1: Build in release mode**

Run: `cargo build --release`
Expected: Success

**Step 2: Run application**

Run: `cargo run --release`
Expected: App starts normally

**Step 3: Play a track**

Use the app to play a track from favorites or history.

**Step 4: Verify MPRIS metadata**

In a separate terminal, run:
```bash
playerctl -l
```
Expected: Should see "biu-tui" in the list

Run:
```bash
playerctl -p biu-tui metadata
```
Expected: Should show track title and artist

Run:
```bash
playerctl -p biu-tui status
```
Expected: Should show "Playing"

**Step 5: Test MPRIS controls**

Test play/pause:
```bash
playerctl -p biu-tui pause
playerctl -p biu-tui status
```
Expected: Status shows "Paused"

```bash
playerctl -p biu-tui play
playerctl -p biu-tui status
```
Expected: Status shows "Playing"

Test next/previous:
```bash
playerctl -p biu-tui next
```
Expected: Next track plays

```bash
playerctl -p biu-tui previous
```
Expected: Previous track plays

**Step 6: Test media keys**

Use keyboard media keys (if available):
- Play/Pause key should toggle playback
- Next/Previous keys should change tracks

**Step 7: Check desktop integration**

- GNOME: Check top bar media indicator shows track info
- KDE: Check media widget in system tray

---

## Task 12: Final cleanup and documentation

**Files:**
- N/A

**Step 1: Run all quality checks**

Run:
```bash
cargo fmt
cargo clippy -- -D warnings
cargo test
```
Expected: All pass

**Step 2: Commit final state**

```bash
git add -A
git commit -m "feat: complete MPRIS integration"
```

**Step 3: Update design doc if needed**

If any changes were made during implementation, update the design document at `docs/plans/2026-03-02-mpris-integration-design.md` to reflect the final implementation.

---

## Summary

This implementation adds full MPRIS support to biu-tui through 12 tasks:

1. Add mpris dependency
2. Create MprisManager struct and API
3. Export mpris module
4. Add mpris field to App
5. Update MPRIS on track change
6. Update MPRIS on playback state changes
7. Poll and handle MPRIS commands
8. Update MPRIS position in main loop
9. Add advance_to_previous to PlayingListManager
10. Run clippy and fix warnings
11. Test MPRIS integration
12. Final cleanup and documentation

Each task follows TDD principles where applicable, includes verification steps, and commits frequently.
