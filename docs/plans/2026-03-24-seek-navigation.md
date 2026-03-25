# Seek Navigation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add seek forward/backward functionality with keyboard shortcuts `h`/`←` and `l`/`→`.

**Architecture:** Add seek capability to AudioDecoder using FFmpeg's seek, communicate seek requests via channel to decoder thread, handle keyboard input in App.

**Tech Stack:** Rust, FFmpeg (ffmpeg-next), crossbeam channels for thread communication

---

### Task 1: Add Seek Method to AudioDecoder

**Files:**
- Modify: `src/audio/decoder.rs`

**Step 1: Add seek method to AudioDecoder**

Add this method to the `AudioDecoder` implementation block:

```rust
pub fn seek(&mut self, timestamp: Duration) -> Result<()> {
    let ts = timestamp.as_secs() as i64 + (timestamp.subsec_nanos() as i64) / 1_000_000_000;
    self.input.seek(ts)?;
    Ok(())
}
```

Add the import at the top of the file (already exists, but ensure it's there):
```rust
use std::time::Duration;
```

**Step 2: Run tests to verify**

Run: `cargo test --lib`
Expected: All tests pass

**Step 3: Commit**

```bash
git add src/audio/decoder.rs
git commit -m "feat: add seek method to AudioDecoder"
```

---

### Task 2: Add Seek Request Channel to AudioPlayer

**Files:**
- Modify: `src/audio/player.rs`

**Step 1: Add SeekCommand enum andchannel**

Add after the imports, before `PlayerState`:

```rust
#[derive(Debug, Clone, Copy)]
pub enum SeekCommand {
    Forward(Duration),
    Backward(Duration),
    To(Duration),
}
```

**Step 2: Add seek_channel to AudioPlayer struct**

Modify `AudioPlayer` struct to add seek_channel field:

```rust
use std::sync::mpsc::{self, Receiver, Sender};

pub struct AudioPlayer {
    state: Arc<Mutex<PlayerState>>,
    position: Arc<Mutex<Duration>>,
    duration: Arc<Mutex<Duration>>,
    volume: Arc<Mutex<f32>>,
    sample_rate: Arc<Mutex<u32>>,
    playback_speed: Arc<Mutex<f32>>,
    stream: Arc<Mutex<Option<cpal::Stream>>>,
    audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    seek_receiver: Mutex<Option<Receiver<SeekCommand>>>,
    seek_sender: Sender<SeekCommand>,
    _decoder_thread: Option<std::thread::JoinHandle<()>>,
}
```

**Step 3: Initialize channel in AudioPlayer::new()**

Modify the `new()` method:

```rust
pub fn new() -> Result<Self> {
    let (seek_sender, seek_receiver) = mpsc::channel();
    Ok(Self {
        state: Arc::new(Mutex::new(PlayerState::Stopped)),
        position: Arc::new(Mutex::new(Duration::ZERO)),
        duration: Arc::new(Mutex::new(Duration::ZERO)),
        volume: Arc::new(Mutex::new(1.0)),
        sample_rate: Arc::new(Mutex::new(44100)),
        playback_speed: Arc::new(Mutex::new(1.0)),
        stream: Arc::new(Mutex::new(None)),
        audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
        seek_receiver: Mutex::new(Some(seek_receiver)),
        seek_sender,
        _decoder_thread: None,
    })
}
```

**Step 4: Handle seek in decoder thread**

Modify the decoder thread in `play()` method to handle seek commands. Find the decoder thread spawn and modify the loop to check forseek commands:

Add after the imports:
```rust
use std::sync::mpsc::TryRecvError;
```

In the decoder thread, modify the main loop to check for seek commands. Replace the existing loop structure:

```rust
let seek_receiver_clone = self.seek_receiver.lock().take();
// ... inside decoder thread spawn ...
loop {
    // Check for seek commands
    if let Some(ref rx) = seek_receiver_clone {
        match rx.try_recv() {
            Ok(SeekCommand::To(pos)) => {
                *position_arc.lock() = pos;
                audio_buffer.lock().clear();
                if let Err(e) = decoder.seek(pos) {
                    eprintln!("Seek failed: {}", e);
                }
                total_samples_decoded = 0;
            }
            Ok(SeekCommand::Forward(delta)) => {
                let current = *position_arc.lock();
                let duration = *duration_arc.lock();
                let new_pos = std::cmp::min(current + delta, duration);
                *position_arc.lock() = new_pos;
                audio_buffer.lock().clear();
                if let Err(e) = decoder.seek(new_pos) {
                    eprintln!("Seek failed: {}", e);
                }
                total_samples_decoded = 0;
            }
            Ok(SeekCommand::Backward(delta)) => {
                let current = *position_arc.lock();
                let new_pos = if current > delta { current - delta } else { Duration::ZERO };
                *position_arc.lock() = new_pos;
                audio_buffer.lock().clear();
                if let Err(e) = decoder.seek(new_pos) {
                    eprintln!("Seek failed: {}", e);
                }
                total_samples_decoded = 0;
            }
            Err(TryRecvError::Empty) => {}
            Err(TryRecvError::Disconnected) => break,
        }
    }
    
    // ... rest of existing loop ...
}
```

**Step 5: Add seek methods to AudioPlayer**

Add these public methods to AudioPlayer:

```rust
pub fn seek_forward(&self, delta: Duration) {
    let _ = self.seek_sender.send(SeekCommand::Forward(delta));
}

pub fn seek_backward(&self, delta: Duration) {
    let _= self.seek_sender.send(SeekCommand::Backward(delta));
}

pub fn seek_to(&self, position: Duration) {
    let _ = self.seek_sender.send(SeekCommand::To(position));
}
```

**Step 6: Export SeekCommand**

Modify `src/audio/mod.rs` to export SeekCommand:

```rust
pub mod decoder;
pub mod player;

pub use decoder::AudioDecoder;
pub use player::{AudioPlayer, PlayerState, SeekCommand};
```

**Step 7: Run tests to verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 8: Commit**

```bash
git add src/audio/player.rs src/audio/mod.rs
git commit -m "feat: add seek channel and methods to AudioPlayer"
```

---

### Task 3: Add Keyboard Handlers in App

**Files:**
- Modify: `src/app.rs`

**Step 1: Add seek helper method to App**

Add this helper method to the `App` impl block (after `apply_volume`):

```rust
fn handle_seek_forward(&mut self) -> Result<()> {
    if let Some(player) = &self.player {
        let current_pos = player.position();
        let duration = player.duration();
        let new_pos = current_pos + Duration::from_secs(5);
        
        if new_pos >= duration {
            match self.settings.loop_mode {
                LoopMode::LoopList => {
                    let next_item = self.playing_list.lock().advance_to_next().cloned();
                    if let Some(item) = next_item {
                        self.play_playlist_item(&item)?;
                    } else if!self.playing_list.lock().items().is_empty() {
                        self.playing_list.lock().jump_to(0);
                        if let Some(item) = self.playing_list.lock().current().cloned() {
                            self.play_playlist_item(&item)?;
                        }
                    }
                }
                LoopMode::NoLoop | LoopMode::LoopOne => {
                    player.stop();
                    if let Some(mpris) = &self.mpris {
                        mpris.set_state(PlayerState::Stopped);
                    }
                }
            }
        } else {
            player.seek_forward(Duration::from_secs(5));
        }
    }
    Ok(())
}

fn handle_seek_backward(&mut self) -> Result<()> {
    if let Some(player) = &self.player {
        let current_pos = player.position();
        
        if current_pos < Duration::from_secs(2) {
            let items_count = self.playing_list.lock().items().len();
            if items_count == 0 {
                return Ok(());
            }
            
            let current_idx = self.playing_list.lock().current_index().unwrap_or(0);
            
            let prev_idx = if current_idx == 0 {
                items_count - 1
            } else {
                current_idx - 1
            };
            
            self.playing_list.lock().jump_to(prev_idx);
            
            if let Some(item) = self.playing_list.lock().current().cloned() {
                let old_now_playing = if let Screen::Library(lib) = &self.screen {
                    lib.now_playing.clone()
                } else {
                    None
                };
                
                self.play_playlist_item(&item)?;
                
                if let Screen::Library(lib) = &mut self.screen {
                    let new_now_playing = lib.now_playing.clone();
                    self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                }
            }
        } else {
            player.seek_backward(Duration::from_secs(5));
        }
    }
    Ok(())
}
```

**Step 2: Add keyboard handlers in handle_key**

In the `handle_key` method, in the `Screen::Library(library)` match arm, add handlers for `h`/`Left` and `l`/`Right` after the existing key handlers (after the `KeyCode::Char(' ')` handler):

```rust
KeyCode::Char('h') | KeyCode::Left => {
    self.handle_seek_backward()?;
}
KeyCode::Char('l') | KeyCode::Right => {
    self.handle_seek_forward()?;
}
```

**Step 3: Run build to verify**

Run: `cargo build`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: add seek forward/backward keyboard handlers"
```

---

### Task 4: Manual Testing

**Step 1: Build and run**

Run: `cargo run --release`

**Step 2: Test seek backward (h/Left)**
- Play a song
- Press `l` a few times toseek forward
- Press `h` to seek backward - should go back5 seconds
- Seek to near the start of song
- Press `h` when position < 2s - should go to previous song
- When at first song, press `h` - should wrap to last song

**Step 3: Test seek forward (l/Right)**
- Play a song
- Press `l` to seek forward - should advance 5 seconds
- Seek near the end of a song
- Test with different loop modes:
  - LoopList: should go to next song (or first if at last)
  - NoLoop/LoopOne: should stop at end

**Step 4: Run tests**

Run: `cargo test`
Expected: All tests pass

**Step 5: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 6: Final commit**

```bash
git add .
git commit -m "feat: implement seek navigation with h/l and arrow keys"
```

---

## Summary

This plan implements seek functionality by:
1. Adding `seek()` method to `AudioDecoder` using FFmpeg's seek capability
2. Adding channel-based communication between main thread and decoder thread for seek commands
3. Adding keyboard handlers in App for `h`/`←` (backward) and `l`/`→` (forward)
4. Implementing smart track navigation based on loop mode and position