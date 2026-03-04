# Playback Speed Control Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add playback speed control (0.5x-2.0x) to biu-tui with FFmpeg atempo filter for pitch-preserving speed adjustment.

**Architecture:** Integrate playback speed at three layers - storage (Settings struct), audio (AudioDecoder with atempo filter), and UI (SettingsScreen). Speed changes during playback restart the track with new speed.

**Tech Stack:** Rust, FFmpeg (ffmpeg-next), serde, ratatui

---

## Task 1: Add playback_speed to Settings struct

**Files:**
- Modify: `src/storage/settings.rs:39-52`
- Test: `src/storage/settings.rs:108-124` (add tests to existing test module)

**Step 1: Write failing tests for playback speed**

Add to the test module in `src/storage/settings.rs` after line 123:

```rust
#[test]
fn test_playback_speed_default() {
    let settings = Settings::default();
    assert_eq!(settings.playback_speed, 1.0);
}

#[test]
fn test_playback_speed_up_increments_correctly() {
    let mut settings = Settings::default();
    settings.playback_speed = 1.0;
    settings.speed_up();
    assert_eq!(settings.playback_speed, 1.1);
}

#[test]
fn test_playback_speed_down_decrements_correctly() {
    let mut settings = Settings::default();
    settings.playback_speed = 1.5;
    settings.speed_down();
    assert_eq!(settings.playback_speed, 1.4);
}

#[test]
fn test_playback_speed_clamped_to_max() {
    let mut settings = Settings::default();
    settings.playback_speed = 1.95;
    settings.speed_up();
    assert_eq!(settings.playback_speed, 2.0);
    settings.speed_up(); // Try to go above max
    assert_eq!(settings.playback_speed, 2.0);
}

#[test]
fn test_playback_speed_clamped_to_min() {
    let mut settings = Settings::default();
    settings.playback_speed = 0.55;
    settings.speed_down();
    assert_eq!(settings.playback_speed, 0.5);
    settings.speed_down(); // Try to go below min
    assert_eq!(settings.playback_speed, 0.5);
}

#[test]
fn test_playback_speed_serialization() {
    let mut settings = Settings::default();
    settings.playback_speed = 1.5;
    let json = serde_json::to_string(&settings).unwrap();
    assert!(json.contains("\"playback_speed\":1.5"));
    
    let deserialized: Settings = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.playback_speed, 1.5);
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_playback_speed --no-fail-fast`
Expected: Compilation errors - `playback_speed` field and methods don't exist

**Step 3: Add playback_speed field to Settings struct**

Modify `src/storage/settings.rs` lines 39-52:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub loop_mode: LoopMode,
    pub playback_speed: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 100,
            loop_mode: LoopMode::default(),
            playback_speed: 1.0,
        }
    }
}
```

**Step 4: Add speed adjustment methods to Settings**

Add to `src/storage/settings.rs` after line 104 (after `volume_float()` method):

```rust
pub fn speed_up(&mut self) {
    self.playback_speed = (self.playback_speed + 0.1).min(2.0);
    let _ = self.save();
}

pub fn speed_down(&mut self) {
    self.playback_speed = (self.playback_speed - 0.1).max(0.5);
    let _ = self.save();
}

pub fn set_playback_speed(&mut self, speed: f32) {
    self.playback_speed = speed.clamp(0.5, 2.0);
    let _ = self.save();
}
```

**Step 5: Run tests to verify they pass**

Run: `cargo test test_playback_speed`
Expected: All 6 tests pass

**Step 6: Commit**

```bash
git add src/storage/settings.rs
git commit -m "feat: add playback_speed field to Settings"
```

---

## Task 2: Update AudioDecoder to support speed parameter

**Files:**
- Modify: `src/audio/decoder.rs`
- Test: No new tests (integration testing done manually)

**Step 1: Read current decoder implementation**

Read: `src/audio/decoder.rs` (entire file)
Understand the current FFmpeg command construction

**Step 2: Add playback_speed field to AudioDecoder struct**

Add to `src/audio/decoder.rs` struct definition:

```rust
pub struct AudioDecoder {
    decoder: ffmpeg_next::decoder::Audio,
    resampler: ffmpeg_next::software::scaling::Context,
    in_channel_layout: ffmpeg_next::ChannelLayout,
    output_sample_rate: u32,
    duration: Duration,
    channels: u16,
    playback_speed: f32, // NEW
}
```

**Step 3: Create new constructor with speed parameter**

Add to `src/audio/decoder.rs` after the existing `from_url_with_sample_rate` method:

```rust
pub fn from_url_with_sample_rate_and_speed(
    url: &str,
    sample_rate: u32,
    speed: f32,
) -> Result<Self> {
    let speed = speed.clamp(0.5, 2.0);
    
    // Build FFmpeg command with atempo filter
    let mut ictx = ffmpeg_next::format::input(&url)?;
    let input = ictx
        .streams()
        .best(ffmpeg_next::media::Type::Audio)
        .context("Could not find audio stream")?;
    let stream_index = input.index();
    
    let context_decoder = ffmpeg_next::codec::context::Context::from_parameters(input.parameters())?;
    let mut decoder = context_decoder.decoder().audio()?;
    
    let in_channel_layout = decoder.channel_layout();
    let channels = decoder.channels();
    
    let output_sample_rate = sample_rate;
    
    // Build filter graph with atempo
    let filter_spec = format!(
        "atempo={},aformat=sample_rates={}:channel_layouts=stereo",
        speed, output_sample_rate
    );
    
    let mut filter_graph = ffmpeg_next::filter::Graph::new();
    let args = &ffmpeg_next::filter::Args {
        filter: &filter_spec,
        inputs: &[],
        outputs: &[],
    };
    
    // Parse the filter
    filter_graph.parse(args)?;
    
    // Get duration from input
    let duration = ictx.duration();
    let duration = Duration::from_secs_f64(duration as f64 / f64::from(ffmpeg_next::ffi::AV_TIME_BASE));
    
    // Setup resampler
    let resampler = ffmpeg_next::software::scaling::Context::get(
        decoder.format(),
        decoder.width(),
        decoder.height(),
        ffmpeg_next::format::Pixel::GRAY8,
        decoder.width(),
        decoder.height(),
        ffmpeg_next::software::scaling::Flags::BILINEAR,
    )?;
    
    Ok(Self {
        decoder,
        resampler,
        in_channel_layout,
        output_sample_rate,
        duration,
        channels,
        playback_speed: speed,
    })
}
```

**Step 4: Update existing constructor to call new one**

Modify the existing `from_url_with_sample_rate` method:

```rust
pub fn from_url_with_sample_rate(url: &str, sample_rate: u32) -> Result<Self> {
    Self::from_url_with_sample_rate_and_speed(url, sample_rate, 1.0)
}
```

**Step 5: Run cargo check to verify compilation**

Run: `cargo check`
Expected: Compilation succeeds or clear errors to fix

**Step 6: Commit**

```bash
git add src/audio/decoder.rs
git commit -m "feat: add speed parameter to AudioDecoder with atempo filter"
```

---

## Task 3: Update AudioPlayer to accept speed parameter

**Files:**
- Modify: `src/audio/player.rs:66-206`

**Step 1: Read current player implementation**

Read: `src/audio/player.rs` lines 66-206 (the `play` method)

**Step 2: Add speed field to AudioPlayer struct**

Modify `src/audio/player.rs` lines 17-26:

```rust
pub struct AudioPlayer {
    state: Arc<Mutex<PlayerState>>,
    position: Arc<Mutex<Duration>>,
    duration: Arc<Mutex<Duration>>,
    volume: Arc<Mutex<f32>>,
    sample_rate: Arc<Mutex<u32>>,
    playback_speed: Arc<Mutex<f32>>, // NEW
    _stream: Option<cpal::Stream>,
    audio_buffer: Arc<Mutex<VecDeque<i16>>>,
    _decoder_thread: Option<std::thread::JoinHandle<()>>,
}
```

**Step 3: Update AudioPlayer::new()**

Modify `src/audio/player.rs` lines 33-44:

```rust
pub fn new() -> Result<Self> {
    Ok(Self {
        state: Arc::new(Mutex::new(PlayerState::Stopped)),
        position: Arc::new(Mutex::new(Duration::ZERO)),
        duration: Arc::new(Mutex::new(Duration::ZERO)),
        volume: Arc::new(Mutex::new(1.0)),
        sample_rate: Arc::new(Mutex::new(44100)),
        playback_speed: Arc::new(Mutex::new(1.0)), // NEW
        _stream: None,
        audio_buffer: Arc::new(Mutex::new(VecDeque::new())),
        _decoder_thread: None,
    })
}
```

**Step 4: Add speed getter and setter methods**

Add to `src/audio/player.rs` after line 64 (after `set_volume` method):

```rust
pub fn playback_speed(&self) -> f32 {
    *self.playback_speed.lock()
}

pub fn set_playback_speed(&self, speed: f32) {
    *self.playback_speed.lock() = speed.clamp(0.5, 2.0);
}
```

**Step 5: Modify play() method to use speed**

Modify `src/audio/player.rs` line 66:

```rust
pub fn play(&mut self, url: &str) -> Result<()> {
```

Change to:

```rust
pub fn play(&mut self, url: &str, speed: f32) -> Result<()> {
    self.set_playback_speed(speed);
```

**Step 6: Update decoder thread to use speed**

Modify `src/audio/player.rs` lines 86-89:

```rust
let decoder_thread = std::thread::spawn(move || {
    if let Ok(mut decoder) =
        AudioDecoder::from_url_with_sample_rate(&url_owned, sample_rate)
    {
```

Change to:

```rust
let speed_for_thread = *self.playback_speed.lock();
let decoder_thread = std::thread::spawn(move || {
    if let Ok(mut decoder) =
        AudioDecoder::from_url_with_sample_rate_and_speed(&url_owned, sample_rate, speed_for_thread)
    {
```

**Step 7: Run cargo check**

Run: `cargo check`
Expected: Compilation errors in app.rs (play() calls need updating)

**Step 8: Commit**

```bash
git add src/audio/player.rs
git commit -m "feat: add speed parameter to AudioPlayer"
```

---

## Task 4: Update App to pass speed to AudioPlayer

**Files:**
- Modify: `src/app.rs`

**Step 1: Find all calls to player.play()**

Run: `rg "player\.play\(" src/app.rs`
Expected: Find locations where play() is called

**Step 2: Update play() calls to include speed**

For each call to `player.play(&url)`, change to:
```rust
player.play(&url, self.settings.playback_speed)
```

**Step 3: Run cargo check**

Run: `cargo check`
Expected: Compilation succeeds

**Step 4: Commit**

```bash
git add src/app.rs
git commit -m "feat: pass playback_speed to AudioPlayer from App"
```

---

## Task 5: Add PlaybackSpeed to SettingsScreen

**Files:**
- Modify: `src/screens/settings.rs`
- Test: `src/screens/settings.rs` (add tests to existing file)

**Step 1: Write failing tests for PlaybackSpeed UI**

Add to `src/screens/settings.rs` at the bottom (create test module if it doesn't exist):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_setting_item_navigation_with_three_items() {
        assert_eq!(SettingItem::Volume.next(), SettingItem::PlaybackSpeed);
        assert_eq!(SettingItem::PlaybackSpeed.next(), SettingItem::LoopMode);
        assert_eq!(SettingItem::LoopMode.next(), SettingItem::Volume);
        
        assert_eq!(SettingItem::Volume.prev(), SettingItem::LoopMode);
        assert_eq!(SettingItem::LoopMode.prev(), SettingItem::PlaybackSpeed);
        assert_eq!(SettingItem::PlaybackSpeed.prev(), SettingItem::Volume);
    }

    #[test]
    fn test_settings_screen_includes_playback_speed() {
        let settings = Settings::default();
        let screen = SettingsScreen::new(settings);
        let items = SettingsScreen::build_items(&screen.settings);
        assert_eq!(items.len(), 3);
        assert!(items[1].text().contains("Playback Speed"));
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo test test_setting_item_navigation --no-fail-fast`
Expected: Compilation errors - `SettingItem::PlaybackSpeed` doesn't exist

**Step 3: Add PlaybackSpeed to SettingItem enum**

Modify `src/screens/settings.rs` lines 9-13:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingItem {
    Volume,
    PlaybackSpeed,
    LoopMode,
}
```

**Step 4: Update SettingItem navigation methods**

Modify `src/screens/settings.rs` lines 15-29:

```rust
impl SettingItem {
    pub fn next(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::PlaybackSpeed,
            SettingItem::PlaybackSpeed => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::Volume,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            SettingItem::Volume => SettingItem::LoopMode,
            SettingItem::LoopMode => SettingItem::PlaybackSpeed,
            SettingItem::PlaybackSpeed => SettingItem::Volume,
        }
    }
}
```

**Step 5: Update build_items to display PlaybackSpeed**

Modify `src/screens/settings.rs` lines 75-84:

```rust
fn build_items(settings: &Settings) -> Vec<ListItem<'_>> {
    let volume_text = format!(
        "Volume        {}  {:3}%",
        Self::format_volume_bar(settings.volume),
        settings.volume
    );
    let speed_text = format!(
        "Playback Speed {}  {:.1}x",
        Self::format_speed_bar(settings.playback_speed),
        settings.playback_speed
    );
    let loop_text = format!("Loop Mode     {}", settings.loop_mode.display_name());

    vec![
        ListItem::new(volume_text),
        ListItem::new(speed_text),
        ListItem::new(loop_text),
    ]
}
```

**Step 6: Add format_speed_bar helper method**

Add to `src/screens/settings.rs` after line 95 (after `format_volume_bar`):

```rust
fn format_speed_bar(speed: f32) -> String {
    let bar_width = 20;
    // Map speed from 0.5-2.0 range to 0-20
    let normalized = (speed - 0.5) / 1.5; // 0.5 to 2.0 is range of 1.5
    let filled = (bar_width as f32 * normalized) as usize;
    let filled = filled.min(bar_width);
    let empty = bar_width - filled;

    let filled_chars: String = std::iter::repeat_n('█', filled).collect();
    let empty_chars: String = std::iter::repeat_n('░', empty).collect();
    format!("{}{}", filled_chars, empty_chars)
}
```

**Step 7: Update adjustment methods**

Modify `src/screens/settings.rs` lines 107-119:

```rust
pub fn adjust_up(&mut self) {
    match self.selected_item {
        SettingItem::Volume => self.settings.volume_up(),
        SettingItem::PlaybackSpeed => self.settings.speed_up(),
        SettingItem::LoopMode => self.settings.next_loop_mode(),
    }
}

pub fn adjust_down(&mut self) {
    match self.selected_item {
        SettingItem::Volume => self.settings.volume_down(),
        SettingItem::PlaybackSpeed => self.settings.speed_down(),
        SettingItem::LoopMode => self.settings.prev_loop_mode(),
    }
}
```

**Step 8: Run tests to verify they pass**

Run: `cargo test test_setting_item`
Expected: All tests pass

**Step 9: Commit**

```bash
git add src/screens/settings.rs
git commit -m "feat: add PlaybackSpeed to SettingsScreen UI"
```

---

## Task 6: Integration testing and manual verification

**Files:**
- Test: Manual testing in running application

**Step 1: Build the application**

Run: `cargo build`
Expected: Build succeeds with no warnings

**Step 2: Run the application**

Run: `cargo run`
Expected: Application starts successfully

**Step 3: Test settings screen UI**

Manual test:
1. Navigate to Settings screen (press 's')
2. Verify three items are displayed: Volume, Playback Speed, Loop Mode
3. Navigate to Playback Speed item (press 'j' or 'k')
4. Adjust speed up (press 'l') - verify speed increases by 0.1x
5. Adjust speed down (press 'h') - verify speed decreases by 0.1x
6. Verify speed is clamped between 0.5x and 2.0x
7. Verify visual bar updates correctly

**Step 4: Test playback with different speeds**

Manual test:
1. Play a track
2. Change playback speed while track is playing
3. Verify track restarts with new speed
4. Verify audio pitch is maintained (no chipmunk effect at 2.0x, no slowdown effect at 0.5x)
5. Test at 0.5x, 1.0x, 1.5x, and 2.0x speeds

**Step 5: Test persistence**

Manual test:
1. Set playback speed to 1.5x
2. Exit application
3. Restart application
4. Navigate to Settings
5. Verify playback speed is still 1.5x

**Step 6: Run all tests**

Run: `cargo test`
Expected: All tests pass

**Step 7: Run clippy**

Run: `cargo clippy -- -D warnings`
Expected: No warnings

**Step 8: Final commit**

```bash
git add -A
git commit -m "test: verify playback speed integration works correctly"
```

---

## Task 7: Update documentation

**Files:**
- Modify: `README.md` (if it exists)

**Step 1: Check if README exists**

Run: `ls README.md`
If exists, continue; if not, skip this task

**Step 2: Add playback speed to features list**

Add to README features section:
```markdown
- Playback speed control (0.5x - 2.0x) with pitch preservation
```

**Step 3: Add usage instructions**

Add to README usage section:
```markdown
### Settings

Press `s` to open settings, then:
- `j/k` - Navigate between settings
- `h/l` - Adjust selected setting
- `Esc` or `s` - Return to library

**Playback Speed:** Adjust from 0.5x to 2.0x in 0.1x increments. Changes take effect immediately.
```

**Step 4: Commit**

```bash
git add README.md
git commit -m "docs: add playback speed to README"
```

---

## Summary

This implementation adds playback speed control to biu-tui with:
- Settings storage with persistence
- FFmpeg atempo filter for pitch-preserving speed adjustment
- Settings screen UI with visual feedback
- Comprehensive test coverage
- Speed range: 0.5x to 2.0x in 0.1x increments
- Default speed: 1.0x

**Total estimated time:** 1-2 hours

**Key dependencies:**
- Task 1 must complete before Task 4 (Settings must have playback_speed field)
- Task 2 must complete before Task 3 (Decoder must accept speed parameter)
- Task 3 must complete before Task 4 (Player must accept speed parameter)
- Task 5 can be done in parallel with Tasks 2-4
- Task 6 must be done after all other tasks
