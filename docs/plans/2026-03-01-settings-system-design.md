# Settings System Design

## Summary

Add a settings system with a dedicated settings screen for configuring:
1. Volume control (0-100%)
2. Loop behavior (Loop Folder / Loop One / No Loop)

## Architecture

```rust
// src/storage/settings.rs

pub trait SettingItem: Clone + Send + Sync {
    type Value: Clone + PartialEq;
    
    fn key(&self) -> &str;
    fn display_name(&self) -> &str;
    fn get(&self) -> Self::Value;
    fn set(&self, value: Self::Value);
    fn format_value(&self) -> String;
    fn adjust_up(&mut self);
    fn adjust_down(&mut self);
}

pub struct Settings {
    pub volume: VolumeSetting,
    pub loop_mode: LoopModeSetting,
}

pub enum LoopMode {
    LoopFolder,    // Loop entire folder
    LoopOne,       // Loop single song
    NoLoop,        // Play through folder, stop at end
}
```

Settings stored in `~/.config/biu-tui/settings.json` (separate from config.json which has download/format settings).

## UI Layout

```
┌──────────────────────────────────────────────────────┐
│ Settings                                              │
├──────────────────────────────────────────────────────┤
│                                                       │
│   Volume        ████████████░░░░░░░░░░░░░  65%       │  ← selected
│                                                       │
│   Loop Mode     No Loop                              │  ← not selected
│                                                       │
├──────────────────────────────────────────────────────┤
│ [j/k] Navigate  [h/l] Adjust  [Esc/s] Back           │
└──────────────────────────────────────────────────────┘
```

### Volume Display

- Visual bar + percentage (0-100%)
- `h` decreases by 5%
- `l` increases by 5%

### Loop Mode Display

Text labels that cycle:
- Order: `Loop Folder` → `Loop One` → `No Loop` → (back to Loop Folder)
- `h` cycles backward, `l` cycles forward

## Key Bindings

| Key | Action |
|-----|--------|
| `j/k` | Navigate between settings |
| `h/l` | Adjust current setting (decrease/increase) |
| `Esc` or `s` | Exit settings screen |

## Data Flow

### When settings change:

1. User presses `h` or `l` in settings screen
2. Setting's `adjust_down()` or `adjust_up()` is called
3. Settings auto-save to `settings.json`
4. For volume: `AudioPlayer::set_volume()` is called immediately
5. For loop mode: stored in settings, read by playback logic

### Integration Points

| Component | Change |
|-----------|--------|
| `App` | Add `Settings` instance, handle `s` key to switch to settings screen |
| `Screen` enum | Add `Settings(SettingsScreen)` variant |
| `AudioPlayer` | Read volume from settings on init, already has `set_volume()` |
| `LibraryScreen` | Read `LoopMode` when song ends to decide next action |
| `Config` | No changes (settings are separate file) |

### Playback Behavior on Song End

- `LoopFolder` → restart from first song in folder
- `LoopOne` → replay current song
- `NoLoop` → play next song if available, stop at folder end

## Error Handling

- Settings load failure → use defaults, log warning
- Settings save failure → log error, continue (non-blocking)
- Corrupt settings.json → delete and recreate with defaults

## Files

### New Files

- `src/storage/settings.rs` — Settings struct, SettingItem trait, VolumeSetting, LoopModeSetting
- `src/screens/settings.rs` — SettingsScreen with render and key handling

### Modified Files

- `src/storage/mod.rs` — Export settings module
- `src/screens/mod.rs` — Export SettingsScreen
- `src/app.rs` — Add Settings, Screen::Settings variant, `s` key handling
- `src/screens/library.rs` — Implement loop behavior in playback

## Testing

Manual testing:
- Verify settings persist between app restarts
- Verify volume applies immediately to audio player
- Verify loop modes work correctly during playback

Settings file location: `~/.config/biu-tui/settings.json`
