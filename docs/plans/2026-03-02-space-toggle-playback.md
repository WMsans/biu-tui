# Space Key Toggle Playback Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add Space key binding to toggle playback: pause when playing, resume when paused, start playback from playing list when stopped.

**Architecture:** Add Space key handler in `handle_key()` that checks player state and calls appropriate methods. Reuses existing pause/resume/play functionality.

**Tech Stack:** Rust, matches existing key handling pattern in `app.rs`

---

### Task 1: Add Space key binding for toggle playback

**Files:**
- Modify: `src/app.rs:191-249` (Library screen key handler)

**Step 1: Add Space key handler in Library screen match block**

Add this case after the `'s'` key handler (around line 248):

```rust
KeyCode::Char(' ') => {
    self.toggle_playback()?;
}
```

**Step 2: Add the toggle_playback method to App**

Add this method to the `impl App` block (after `play_playlist_item` around line 420):

```rust
fn toggle_playback(&mut self) -> Result<()> {
    if let Some(player) = &self.player {
        match player.state() {
            PlayerState::Playing => {
                player.pause();
            }
            PlayerState::Paused => {
                player.resume();
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

fn start_playback_if_available(&mut self) -> Result<()> {
    let item = {
        let mut list = self.playing_list.lock();
        if list.items().is_empty() {
            return Ok(());
        }
        if list.current().is_none() {
            list.jump_to(0);
        }
        list.current().cloned()
    };

    if let Some(item) = item {
        self.play_playlist_item(&item)?;
    }
    Ok(())
}
```

**Step 3: Verify compilation**

Run: `cargo check`
Expected: No errors

**Step 4: Test manually**

Run: `cargo run`
Expected:
- When playing: Space pauses
- When paused: Space resumes
- When stopped: Space starts playback from playing list (if not empty)

**Step 5: Commit**

```bash
git add src/app.rs
git commit -m "feat: add Space key to toggle playback"
```
