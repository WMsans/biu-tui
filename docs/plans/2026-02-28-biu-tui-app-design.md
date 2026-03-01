# Biu TUI App Design

## Overview

A terminal-based Bilibili music player built with Rust + Ratatui, supporting high-quality audio playback and audio extraction downloads.

## Features

| Feature   | Implementation                                    |
| --------- | ------------------------------------------------- |
| Login     | QR code + SMS, cookies persisted                  |
| Library   | Favorites, Watch Later, History                   |
| Playback  | FFmpeg decode → cpal output, Hi-Res/FLAC/192K     |
| Downloads | Batch download, audio extraction to FLAC/MP3/Opus |
| UI        | Ratatui, keyboard-driven, now-playing bar         |

## Architecture

```
biu-tui/
├── Cargo.toml
├── src/
│   ├── main.rs              # Entry point, tokio runtime setup
│   ├── app.rs               # App state, screen routing, event loop
│   ├── lib.rs
│   │
│   ├── ui/
│   │   ├── mod.rs
│   │   ├── theme.rs         # Colors, styles
│   │   └── widgets/         # Reusable widgets (progress bar, list, etc.)
│   │
│   ├── screens/
│   │   ├── mod.rs
│   │   ├── login.rs         # QR code display, SMS input
│   │   ├── library.rs       # Favorites, Watch Later, History tabs
│   │   ├── player.rs        # Now playing, controls
│   │   ├── download.rs      # Download queue, progress
│   │   └── settings.rs      # Output format, quality preferences
│   │
│   ├── api/
│   │   ├── mod.rs
│   │   ├── client.rs        # HTTP client with cookie handling
│   │   ├── auth.rs          # Login APIs (QR, SMS)
│   │   ├── favorite.rs      # Favorites APIs
│   │   ├── history.rs       # History APIs
│   │   ├── player.rs        # Playurl APIs (audio stream URLs)
│   │   └── types.rs         # API response types
│   │
│   ├── audio/
│   │   ├── mod.rs
│   │   ├── player.rs        # FFmpeg-based playback engine
│   │   └── stream.rs        # Stream decoding
│   │
│   ├── download/
│   │   ├── mod.rs
│   │   ├── manager.rs       # Concurrent download queue
│   │   └── extractor.rs     # Audio extraction via FFmpeg
│   │
│   └── storage/
│       ├── mod.rs
│       ├── config.rs        # User preferences
│       └── cookies.rs       # Session persistence
```

## Screen Flow

```
┌─────────┐    Login     ┌──────────┐
│  Login  │ ──────────► │ Library  │
└─────────┘              └──────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │ Favorites│   │WatchLater│   │ History  │
        └──────────┘   └──────────┘   └──────────┘
              │               │               │
              └───────────────┴───────────────┘
                              │
                              ▼
                       ┌──────────┐
                       │  Player  │◄─────── Now Playing Bar
                       └──────────┘
                              │
                              ▼
                       ┌──────────┐
                       │ Download │
                       └──────────┘
```

## UI Layout (Library Screen)

```
┌─────────────────────────────────────────────────────────────┐
│ Biu TUI                                    [Tab] Navigate   │
├─────────────────────────────────────────────────────────────┤
│ [Favorites]  [Watch Later]  [History]                       │
├─────────────────────────────────────────────────────────────┤
│ ┌─────────────────────────────────────────────────────────┐ │
│ │  ▶ Song Title A         Artist      4:32    [HQ]        │ │
│ │    Album Name                                           │ │
│ ├─────────────────────────────────────────────────────────┤ │
│ │    Song Title B         Artist      3:45                │ │
│ │    Album Name                                           │ │
│ ├─────────────────────────────────────────────────────────┤ │
│ │    Song Title C         Artist      5:01    [FLAC]      │ │
│ │    Album Name                                           │ │
│ └─────────────────────────────────────────────────────────┘ │
├─────────────────────────────────────────────────────────────┤
│ ▶ Now Playing: Song Title A    1:23 / 4:32    ██████░░░░   │
│ [Space] Play/Pause  [D] Download  [Q] Queue                 │
└─────────────────────────────────────────────────────────────┘
```

## Key Bindings

| Key            | Action                          |
| -------------- | ------------------------------- |
| `j/k` or `↑/↓` | Navigate list                   |
| `h/l` or `←/→` | Seek backward/forward 5 seconds |
| `Enter`        | Play selected                   |
| `Space`        | Play/Pause                      |
| `n/p`          | Next/Previous track             |
| `d`            | Download current                |
| `D`            | Batch download (favorites)      |
| `Tab`          | Switch tabs                     |
| `1/2/3`        | Switch screens                  |
| `q`            | Add to queue                    |
| `?`            | Help                            |
| `Esc`          | Back/Close modal                |

## Audio System

### Audio Pipeline

```
Bilibili API → Stream URL → FFmpeg Decoder → cpal Audio Output
                   │
                   └─► Quality Selection:
                       1. Hi-Res / Dolby (if available)
                       2. FLAC (lossless)
                       3. 192K AAC
                       4. 128K AAC (fallback)
```

### Key Components

1. **Stream Fetcher** - Gets audio URLs from Bilibili playurl API, selects highest quality
2. **FFmpeg Decoder** - Decodes any format (FLAC, AAC, Opus) to PCM
3. **cpal Output** - Cross-platform audio output (PulseAudio, ALSA, CoreAudio, WASAPI)
4. **Playback Controller** - Play, pause, seek, volume, next/previous

## Download System

### Download Flow

```
Video/Audio URL → HTTP Download → FFmpeg Extract → Encode → Save
                                       │
                                       └─► Output Format:
                                           - FLAC (lossless, from FLAC source)
                                           - MP3 (320K, for compatibility)
                                           - Opus (efficient, good quality)
```

### Batch Download (Favorites)

1. Select favorite folder
2. Choose output format
3. Download all items with progress bar
4. Skip already downloaded (by BVID)

### Audio Extraction via FFmpeg

```bash
ffmpeg -i input_video.mp4 -vn -c:a flac output.flac
ffmpeg -i input_video.mp4 -vn -c:a libmp3lame -b:a 320k output.mp3
```

## Authentication System

### Login Methods

**1. QR Code Login (Primary)**

- Generate QR code via Bilibili API
- Display as ASCII/Unicode art in terminal
- Poll for scan confirmation
- Store cookies on success

**2. SMS Login (Fallback)**

- Input phone number (with country code)
- Send verification code
- Enter code to complete login

### Session Persistence

- Cookies stored in `~/.config/biu-tui/cookies.json`
- Auto-refresh session on app start
- Validate login status on startup

## Dependencies

```toml
[dependencies]
ratatui = "0.29"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies"] }
ffmpeg-next = "7"
cpal = "0.15"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
qrcode = "0.14"
anyhow = "1"
thiserror = "1"
dirs = "6"
```

## API Reference

Based on existing biu-tui service layer:

- `passport-login-web-qrcode-generate` - Generate QR code
- `passport-login-web-qrcode-poll` - Poll for QR scan
- `passport-login-web-sms-send` - Send SMS code
- `passport-login-web-login-sms` - Login with SMS
- `fav-folder-created-list` - Get created favorite folders
- `fav-folder-collected-list` - Get collected favorite folders
- `fav-resource` - Get resources in favorite folder
- `history-toview-list` - Get watch later list
- `web-interface-history` - Get viewing history
- `player-playurl` - Get audio stream URLs
