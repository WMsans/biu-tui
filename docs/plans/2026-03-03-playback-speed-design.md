# Playback Speed Control - Design Document

**Date:** 2026-03-03  
**Author:** AI Assistant  
**Status:** Approved

## Overview

Add playback speed control to biu-tui, allowing users to adjust audio playback speed from 0.5x to 2.0x while maintaining pitch quality using FFmpeg's atempo filter.

## Requirements

- **Speed range:** 0.5x to 2.0x
- **Step size:** 0.1x increments
- **Default speed:** 1.0x (normal playback)
- **UI location:** Settings screen, adjustable with h/l keys
- **Audio quality:** Maintain pitch using FFmpeg atempo filter
- **Persistence:** Save speed preference to settings.json

## Architecture

### Three-Layer Integration

1. **Storage Layer** - Add `playback_speed: f32` to Settings struct
2. **Audio Layer** - Apply FFmpeg atempo filter during decoding
3. **UI Layer** - Display and adjust speed in settings screen

### Data Flow

```
User adjusts speed in Settings 
  → Settings saved to disk 
  → AudioPlayer restarts current track with new speed (if playing)
  
On app startup 
  → Saved speed loaded 
  → Applied to new playback
```

## Data Structures

### Settings Struct (src/storage/settings.rs)

```rust
pub struct Settings {
    pub volume: u32,
    pub loop_mode: LoopMode,
    pub playback_speed: f32,  // NEW: Range 0.5-2.0, default 1.0
}
```

**New methods:**
- `speed_up()` - increase by 0.1 (max 2.0), save to disk
- `speed_down()` - decrease by 0.1 (min 0.5), save to disk
- `set_playback_speed(f32)` - set speed with validation, save to disk

### SettingsScreen Enum (src/screens/settings.rs)

```rust
pub enum SettingItem {
    Volume,         // index 0
    PlaybackSpeed,  // index 1 - NEW
    LoopMode,       // index 2
}
```

**Serialization:** The `playback_speed` field will be serialized as a float in `settings.json`, automatically handled by serde.

## Audio Implementation

### AudioDecoder Modifications (src/audio/decoder.rs)

**Struct changes:**
```rust
pub struct AudioDecoder {
    // existing fields...
    playback_speed: f32,  // NEW
}
```

**New constructor:**
```rust
pub fn from_url_with_sample_rate_and_speed(
    url: &str, 
    sample_rate: u32, 
    speed: f32
) -> Result<Self>
```

**FFmpeg integration:**
- Apply atempo filter: `-af "atempo={speed}"`
- Valid range for atempo is 0.5 to 2.0 (matches requirements)
- Filter applied during resampling pipeline

**Backward compatibility:**
- Keep existing `from_url_with_sample_rate()` method (defaults to 1.0x)

### AudioPlayer Integration (src/audio/player.rs)

**Modifications:**
- Accept speed parameter in `play()` method
- Pass speed to AudioDecoder when creating decoder instance
- When speed changes during playback, stop and restart with new speed

**Note:** The atempo filter is applied during FFmpeg decoding/resampling, so the decoder thread and audio callback remain unchanged.

## UI Implementation

### Settings Screen (src/screens/settings.rs)

**Display format:**
```
Playback Speed  {bar}  {speed:.1}x
```

**Example:**
```
Playback Speed  ████████████░░░░░░░░  1.2x
```

**Specifications:**
- Bar width: 20 characters (same as volume)
- Speed display: 1 decimal place with "x" suffix
- Position: Between Volume and LoopMode items

**Navigation:**
- Cycle through 3 items: Volume → PlaybackSpeed → LoopMode → Volume
- `next()` and `prev()` methods updated for 3-item cycle

**Adjustment:**
- `adjust_up()`: call `settings.speed_up()`
- `adjust_down()`: call `settings.speed_down()`

**Help text:** Unchanged (already covers navigation and adjustment)

### Visual Layout

```
Settings
┌──────────────────────────────────────┐
│ Volume        ████████████████████░░░  95%     │
│ Playback Speed ████████████████░░░░░░  1.2x    │
│ Loop Mode     Loop List                       │
└──────────────────────────────────────┘
[j/k] Navigate  [h/l] Adjust  [Esc/s] Back
```

## Error Handling

### Invalid Speed Values
- Settings setters clamp speed to valid range (0.5-2.0)
- If corrupted JSON loads with out-of-range speed, clamp and save corrected value

### FFmpeg atempo Filter Failures
- Fall back to 1.0x speed if atempo filter fails
- Log warning but don't crash the application

### Settings File Corruption
- If settings.json is corrupted, load defaults (including playback_speed: 1.0)
- Save corrected settings to disk

## Testing Strategy

### Unit Tests (src/storage/settings.rs)
- `test_playback_speed_up_increments_correctly()`
- `test_playback_speed_down_decrements_correctly()`
- `test_playback_speed_clamped_to_range()`
- `test_playback_speed_serialization()`

### Unit Tests (src/screens/settings.rs)
- `test_setting_item_navigation_with_three_items()`
- `test_playback_speed_adjustment()`

### Integration Tests
- Test speed changes persist across app restarts
- Test playback restarts with new speed when changed during playback

### Manual Testing
- Verify audio quality at various speeds (0.5x, 1.0x, 2.0x)
- Verify pitch is maintained (no chipmunk/slowdown effects)
- Verify UI displays correctly

## Edge Cases

1. **Speed adjustment when player is stopped:** No restart needed
2. **Speed adjustment when player is playing:** Restart required with new speed
3. **Loading settings from older version:** Serde default handles missing field
4. **Corrupted speed value:** Clamp to valid range and save

## Implementation Notes

- Speed changes during playback require restarting the track (standard media player behavior)
- FFmpeg's atempo filter maintains pitch quality (no audio artifacts)
- Simple implementation, easy to maintain
- Matches common media player UX patterns
