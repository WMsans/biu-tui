# Space Key Toggle Playback Design

## Summary

Add Space key binding to toggle audio playback: pause when playing, resume when paused, start playback when stopped.

## Behavior

| Current State | Action |
|---------------|--------|
| Playing | Pause |
| Paused | Resume |
| Stopped | Start playing from playing list (if not empty) |

## Implementation

**File:** `src/app.rs`

Add `KeyCode::Char(' ')` handling in `handle_key()` method:
1. Check `self.player.state()`
2. Call appropriate method based on state
3. For stopped state, start playback from playing list using existing play logic

## Scope

- Single file change: `src/app.rs`
- Reuses existing `pause()`, `resume()`, and play methods
- No changes to audio player implementation needed
