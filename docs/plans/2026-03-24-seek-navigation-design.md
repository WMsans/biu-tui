# Seek Navigation Design

**Goal:** Add keyboard shortcuts for seeking forward/backward in audio playback with intelligent track navigation.

## Behavior

### `h` / `←`(Left Arrow)
- Seek backward 5 seconds
- If current position < 2 seconds: go to previous song
- If at first song: wrap to last song in playlist

### `l` / `→` (Right Arrow)
- Seek forward 5 seconds
- If seeking past end of track:
- If loop mode is `LoopList`: go to next song (or first if at last)
  - Otherwise (`NoLoop` or `LoopOne`): stop at end of current track

## Architecture

1. **AudioDecoder** - Add `seek()` method using FFmpeg's input context seek capability
2. **AudioPlayer** - Add channel-based seek communication between main thread and decoder thread
3. **App** - Add keyboard handlers in Library screen for h/l and arrow keys

## Files Modified

- `src/audio/decoder.rs` - Add seek method
- `src/audio/player.rs` - Add seek channel and handling
- `src/app.rs` - Add keyboard handlers

## Implementation Plan

See: `docs/plans/2026-03-24-seek-navigation.md`