# Vim-Style Seek Controls Design

**Date:** 2026-03-06
**Status:** Approved

## Overview

Implement Vim-style navigation (h/l) for seeking within tracks, with hold behavior for continuous seeking backward (h) and 3x speed playback forward (l).

## Requirements

| Key | Action | Behavior |
|-----|--------|----------|
| `h` press | Seek -5s | Jump backward 5 seconds |
| `h` hold | Seek -5s | Repeat every 200ms while held |
| `l` press | Seek +5s | Jump forward 5 seconds |
| `l` hold | Speed 3x | Play at 3x speed while held |
| `l` release | Restore speed | Return to previous speed |

## Architecture

### Component Overview

| Component | File | Purpose |
|-----------|------|---------|
| KeyHoldTracker | `src/input.rs` (new) | Track key press/hold/release state |
| AudioDecoder.seek() | `src/audio/decoder.rs` | FFmpeg av_seek_frame implementation |
| DecoderCommand channel | `src/audio/player.rs` | Async control of decoder thread |
| AudioPlayer.seek() | `src/audio/player.rs` | Public API, buffer sync, seek coordination |
| Event loop updates | `src/app.rs` | Enable keyboard enhancements, handle actions |

### Data Flow

```
User presses 'h'
       |
       v
KeyHoldTracker.process(key)
       |
       v
KeyAction::Press(KeyCode::Char('h'))
       |
       v
AudioPlayer.seek(position - 5s)
       |
       +-> Clear audio_buffer
       +-> Set seek_pending
       +-> Send DecoderCommand::Seek via channel
              |
              v
       Decoder thread receives command
              |
              +-> decoder.seek(target) [FFmpeg av_seek_frame]
              +-> Clear seek_pending, resume decoding
                     |
                     v
              Audio buffer refills, playback resumes
```

## Component Designs

### 1. Key Hold State Tracking

**New module: `src/input.rs`**

```rust
pub struct KeyHoldTracker {
    held_keys: HashSet<KeyCode>,
    last_action: HashMap<KeyCode, Instant>,
    action_interval: Duration,
}

pub enum KeyAction {
    Press(KeyCode),
    Hold(KeyCode),
    Release(KeyCode),
}
```

**Behavior:**
- `KeyEventKind::Press` -> `KeyAction::Press` (first time) or `KeyAction::Hold` (subsequent)
- `KeyEventKind::Release` -> `KeyAction::Release`
- Track action timing for interval-based hold actions

### 2. FFmpeg Decoder Seeking

**Changes to `src/audio/decoder.rs`**

```rust
impl AudioDecoder {
    pub fn seek(&mut self, timestamp: Duration) -> Result<()> {
        let target_pts = av_rescale_q(
            timestamp.as_secs_f64() as i64,
            AV_TIME_BASE_Q,
            self.format_context.streams().get(self.stream_index)?.time_base()
        );
        
        unsafe {
            av_seek_frame(
                self.format_context.as_ptr(),
                self.stream_index,
                target_pts,
                AVSEEK_FLAG_BACKWARD
            );
        }
        
        self.codec_context.flush_buffers();
        Ok(())
    }
}
```

**Decoder Thread Communication:**

```rust
pub enum DecoderCommand {
    Seek(Duration),
    SetSpeed(f32),
    Stop,
}
```

### 3. Audio Buffer Management

**Changes to `src/audio/player.rs`**

```rust
pub struct AudioPlayer {
    // Existing fields...
    seek_pending: Arc<Mutex<Option<Duration>>>,
    command_tx: mpsc::Sender<DecoderCommand>,
}
```

**Seek Sequence:**
1. On seek request: Set `seek_pending`, clear `audio_buffer`, send command
2. Decoder thread: Receives command, calls `decoder.seek()`, clears `seek_pending`
3. Audio output: Output silence while `seek_pending.is_some()`

**Buffer State Machine:**

```
[Playing] --seek()--> [Seeking] --decode--> [Buffering] --buffer full--> [Playing]
```

### 4. Event Loop Integration

**Changes to `src/app.rs`**

Enable crossterm's keyboard enhancement:

```rust
use crossterm::event::{KeyboardEnhancementFlags, PushKeyboardEnhancementFlags};

fn setup_terminal(&mut self) -> Result<()> {
    queue!(
        std::io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::REPORT_EVENT_TYPES
        )
    )?;
    Ok(())
}
```

**Key handling in event loop:**

```rust
let mut key_tracker = KeyHoldTracker::new(Duration::from_millis(200));
let mut previous_speed: Option<f32> = None;

// In event loop:
let action = key_tracker.process(key);

match action {
    KeyAction::Press(KeyCode::Char('h')) | KeyAction::Hold(KeyCode::Char('h')) => {
        // Seek backward 5s
    }
    KeyAction::Press(KeyCode::Char('l')) => {
        // Seek forward 5s
    }
    KeyAction::Hold(KeyCode::Char('l')) => {
        // Set 3x speed, save previous
    }
    KeyAction::Release(KeyCode::Char('l')) => {
        // Restore previous speed
    }
}
```

## Files Changed

| File | Changes |
|------|---------|
| `src/input.rs` | **New** - KeyHoldTracker implementation |
| `src/audio/decoder.rs` | Add `seek()` method using av_seek_frame |
| `src/audio/player.rs` | Add command channel, implement seek(), buffer management |
| `src/audio/mod.rs` | Export new types |
| `src/app.rs` | Enable keyboard enhancements, integrate KeyHoldTracker |
| `Cargo.toml` | Verify crossterm feature flags |

## Error Handling

| Scenario | Handling |
|----------|----------|
| Seek beyond duration | Clamp to duration |
| Seek before 0 | Clamp to 0 |
| Seek while stopped | No-op |
| FFmpeg seek failure | Log error, continue from current position |

## Testing Strategy

- Unit tests for KeyHoldTracker state transitions
- Unit tests for seek position clamping
- Manual testing of hold behavior timing
- Manual testing of audio continuity after seek
