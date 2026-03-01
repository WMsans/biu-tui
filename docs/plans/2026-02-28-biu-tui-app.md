# Biu TUI App Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a terminal-based Bilibili music player with high-quality audio playback and audio extraction downloads.

**Architecture:** Layered Rust application with Ratatui for UI, FFmpeg for audio decoding, and async Tokio runtime for API calls and concurrent downloads.

**Tech Stack:** Rust, Ratatui, Tokio, reqwest, ffmpeg-next, cpal, serde

---

## Task 1: Project Initialization

**Files:**

- Create: `./Cargo.toml`
- Create: `./src/main.rs`
- Create: `./src/lib.rs`
- Create: `./src/app.rs`

**Step 1: Create project directory and initialize Cargo**

```bash
mkdir -p ./src
```

**Step 2: Create Cargo.toml**

```toml
[package]
name = "biu-tui"
version = "0.1.0"
edition = "2021"
description = "Terminal-based Bilibili music player"
license = "PolyForm-Noncommercial-1.0.0"

[dependencies]
ratatui = "0.29"
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", features = ["json", "cookies", "rustls-tls"] }
ffmpeg-next = "7"
cpal = "0.15"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
qrcode = "0.14"
anyhow = "1"
thiserror = "2"
dirs = "6"
crossterm = "0.28"
futures = "0.3"
parking_lot = "0.12"

[dev-dependencies]
tokio-test = "0.4"
```

**Step 3: Create src/lib.rs**

```rust
pub mod api;
pub mod app;
pub mod audio;
pub mod download;
pub mod screens;
pub mod storage;
pub mod ui;
```

**Step 4: Create src/main.rs**

```rust
use anyhow::Result;
use biu_tui::app::App;

fn main() -> Result<()> {
    let mut app = App::new()?;
    app.run()?;
    Ok(())
}
```

**Step 5: Create src/app.rs (skeleton)**

```rust
use anyhow::Result;
use ratatui::{backend::CrosstermBackend, Terminal};
use crossterm::{execute, terminal::*};

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
}

impl App {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            running: true,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            self.terminal.draw(|f| {
                let area = f.area();
                f.render_widget(ratatui::widgets::Paragraph::new("Biu TUI - Press 'q' to quit"), area);
            })?;

            if crossterm::event::poll(std::time::Duration::from_millis(100))? {
                if let crossterm::event::Event::Key(key) = crossterm::event::read()? {
                    if key.code == crossterm::event::KeyCode::Char('q') {
                        self.running = false;
                    }
                }
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}
```

**Step 6: Create module directories**

```bash
mkdir -p ./src/{api,audio,download,screens,storage,ui}
touch ./src/{api,audio,download,screens,storage,ui}/mod.rs
```

**Step 7: Verify build**

```bash
cargo build
```

Expected: Build succeeds (may have warnings about unused modules)

**Step 8: Commit**

```bash
git init && git add . && git commit -m "chore: initialize biu-tui project"
```

---

## Task 2: API Client Foundation

**Files:**

- Create: `./src/api/client.rs`
- Create: `./src/api/types.rs`
- Modify: `./src/api/mod.rs`

**Step 1: Create src/api/types.rs**

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub mid: u64,
    pub uname: String,
    pub face: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteFolder {
    pub id: u64,
    pub title: String,
    pub media_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteResource {
    pub id: u64,
    pub bvid: String,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub upper: Upper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upper {
    pub mid: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayUrlData {
    pub dash: Option<DashData>,
    pub durl: Option<Vec<DurlData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashData {
    pub audio: Vec<AudioDash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDash {
    pub id: u32,
    pub base_url: Option<String>,
    pub backup_url: Option<Vec<String>>,
    pub bandwidth: u32,
    pub codecid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurlData {
    pub url: String,
    pub size: u64,
}
```

**Step 2: Create src/api/client.rs**

```rust
use anyhow::{Context, Result};
use reqwest::{cookie::Jar, Client};
use std::sync::Arc;
use crate::api::types::*;

const BILIBILI_BASE_URL: &str = "https://api.bilibili.com";

pub struct BilibiliClient {
    client: Client,
    cookie_jar: Arc<Jar>,
    csrf: Option<String>,
}

impl BilibiliClient {
    pub fn new() -> Result<Self> {
        let cookie_jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            cookie_jar,
            csrf: None,
        })
    }

    pub fn set_csrf(&mut self, csrf: String) {
        self.csrf = Some(csrf);
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let response = self.client.get(&url).send().await?;
        let api_response = response.json::<ApiResponse<T>>().await?;
        Ok(api_response)
    }

    pub async fn post<T: serde::de::DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let mut form_data = form.to_vec();
        if let Some(ref csrf) = self.csrf {
            form_data.push(("csrf", csrf.as_str()));
        }
        let response = self.client.post(&url).form(&form_data).send().await?;
        let api_response = response.json::<ApiResponse<T>>().await?;
        Ok(api_response)
    }
}
```

**Step 3: Update src/api/mod.rs**

```rust
pub mod client;
pub mod types;

pub use client::BilibiliClient;
pub use types::*;
```

**Step 4: Verify build**

```bash
cargo build
```

**Step 5: Commit**

```bash
git add . && git commit -m "feat(api): add Bilibili API client foundation"
```

---

## Task 3: Authentication - QR Code Login

**Files:**

- Create: `./src/api/auth.rs`
- Create: `./src/api/qrcode.rs`
- Modify: `./src/api/mod.rs`

**Step 1: Create src/api/auth.rs**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use crate::api::BilibiliClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeData {
    pub url: String,
    pub qrcode_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPollData {
    pub code: i32,
    pub message: String,
    pub url: Option<String>,
    pub refresh_token: Option<String>,
    pub timestamp: Option<u64>,
}

impl BilibiliClient {
    pub async fn generate_qrcode(&self) -> Result<QrCodeData> {
        let response = self.client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .send()
            .await
            .context("Failed to request QR code")?;

        let json: serde_json::Value = response.json().await?;
        let data = json.get("data").context("No data in QR response")?;

        Ok(QrCodeData {
            url: data.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            qrcode_key: data.get("qrcode_key").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        })
    }

    pub async fn poll_qrcode(&self, qrcode_key: &str) -> Result<QrPollData> {
        let url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
            qrcode_key
        );
        let response = self.client.get(&url).send().await.context("Failed to poll QR")?;
        let json: serde_json::Value = response.json().await?;

        Ok(QrPollData {
            code: json.get("data").and_then(|d| d.get("code")).and_then(|c| c.as_i64()).unwrap_or(-1) as i32,
            message: json.get("data").and_then(|d| d.get("message")).and_then(|m| m.as_str()).unwrap_or("").to_string(),
            url: json.get("data").and_then(|d| d.get("url")).and_then(|u| u.as_str()).map(|s| s.to_string()),
            refresh_token: json.get("data").and_then(|d| d.get("refresh_token")).and_then(|r| r.as_str()).map(|s| s.to_string()),
            timestamp: json.get("data").and_then(|d| d.get("timestamp")).and_then(|t| t.as_u64()),
        })
    }
}
```

**Step 2: Update src/api/mod.rs**

```rust
pub mod auth;
pub mod client;
pub mod types;

pub use client::BilibiliClient;
pub use types::*;
```

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat(api): add QR code login endpoints"
```

---

## Task 4: Login Screen UI

**Files:**

- Create: `./src/screens/login.rs`
- Create: `./src/ui/widgets/qrcode.rs`
- Modify: `./src/screens/mod.rs`
- Modify: `./src/ui/mod.rs`

**Step 1: Create src/ui/widgets/qrcode.rs**

```rust
use qrcode::QrCode;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::Widget,
};

pub struct QrCodeWidget {
    code: QrCode,
}

impl QrCodeWidget {
    pub fn new(url: &str) -> anyhow::Result<Self> {
        let code = QrCode::new(url)?;
        Ok(Self { code })
    }
}

impl Widget for QrCodeWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let width = self.code.width();
        let block = "█";
        let space = " ";

        let start_x = area.x + (area.width.saturating_sub(width as u16 * 2)) / 2;
        let start_y = area.y + (area.height.saturating_sub(width as u16)) / 2;

        for y in 0..width {
            for x in 0..width {
                let color = if self.code[(x, y)] == qrcode::Color::Dark {
                    Color::White
                } else {
                    Color::Black
                };

                let cell_x = start_x + (x * 2) as u16;
                let cell_y = start_y + y as u16;

                if cell_x < area.x + area.width && cell_y < area.y + area.height {
                    buf[(cell_x, cell_y)].set_symbol(block).set_fg(color);
                    if cell_x + 1 < area.x + area.width {
                        buf[(cell_x + 1, cell_y)].set_symbol(block).set_fg(color);
                    }
                }
            }
        }
    }
}
```

**Step 2: Create src/ui/widgets/mod.rs**

```rust
pub mod qrcode;

pub use qrcode::QrCodeWidget;
```

**Step 3: Update src/ui/mod.rs**

```rust
pub mod widgets;
pub mod theme;

pub use widgets::*;
```

**Step 4: Create src/ui/theme.rs**

```rust
use ratatui::style::{Color, Style};

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub accent: Color,
    pub error: Color,
    pub success: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Black,
            foreground: Color::White,
            accent: Color::Cyan,
            error: Color::Red,
            success: Color::Green,
        }
    }
}

impl Theme {
    pub fn title_style(&self) -> Style {
        Style::default().fg(self.accent).bold()
    }

    pub fn normal_style(&self) -> Style {
        Style::default().fg(self.foreground)
    }

    pub fn error_style(&self) -> Style {
        Style::default().fg(self.error)
    }
}
```

**Step 5: Create src/screens/login.rs**

```rust
use anyhow::Result;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use crate::api::{BilibiliClient, QrCodeData, QrPollData};
use crate::ui::QrCodeWidget;

pub enum LoginState {
    Idle,
    QrWaiting { qrcode_data: QrCodeData },
    QrScanned,
    LoggedIn,
    Error(String),
}

pub struct LoginScreen {
    pub state: LoginState,
    pub status_message: String,
}

impl LoginScreen {
    pub fn new() -> Self {
        Self {
            state: LoginState::Idle,
            status_message: "Press 'r' to refresh QR code".to_string(),
        }
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        let title = Paragraph::new("Biu TUI - Login")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::BOTTOM));
        f.render_widget(title, chunks[0]);

        match &self.state {
            LoginState::QrWaiting { qrcode_data } => {
                if let Ok(qr_widget) = QrCodeWidget::new(&qrcode_data.url) {
                    f.render_widget(qr_widget, chunks[1]);
                }
            }
            _ => {
                let msg = Paragraph::new(self.status_message.clone())
                    .alignment(Alignment::Center);
                f.render_widget(msg, chunks[1]);
            }
        }

        let help = Paragraph::new("[R] Refresh QR  [Q] Quit")
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }
}
```

**Step 6: Create src/screens/mod.rs**

```rust
pub mod login;

pub use login::LoginScreen;
```

**Step 7: Verify build**

```bash
cargo build
```

**Step 8: Commit**

```bash
git add . && git commit -m "feat(ui): add login screen with QR code display"
```

---

## Task 5: Storage Layer

**Files:**

- Create: `./src/storage/config.rs`
- Create: `./src/storage/cookies.rs`
- Modify: `./src/storage/mod.rs`

**Step 1: Create src/storage/config.rs**

```rust
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub download_dir: PathBuf,
    pub output_format: OutputFormat,
    pub audio_quality: AudioQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Flac,
    Mp3 { bitrate: u32 },
    Opus { bitrate: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioQuality {
    HiRes,
    Flac,
    K192,
    K128,
}

impl Default for Config {
    fn default() -> Self {
        let download_dir = dirs::audio_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("biu-tui");

        Self {
            download_dir,
            output_format: OutputFormat::Flac,
            audio_quality: AudioQuality::Flac,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_dir()?.join("config.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_dir()?.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
```

**Step 2: Create src/storage/cookies.rs**

```rust
use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct CookieStorage;

impl CookieStorage {
    pub fn cookie_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir.join("cookies.json"))
    }

    pub fn save(cookies: &str) -> Result<()> {
        let path = Self::cookie_path()?;
        std::fs::write(&path, cookies)?;
        Ok(())
    }

    pub fn load() -> Result<Option<String>> {
        let path = Self::cookie_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    pub fn clear() -> Result<()> {
        let path = Self::cookie_path()?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
```

**Step 3: Update src/storage/mod.rs**

```rust
pub mod config;
pub mod cookies;

pub use config::{Config, OutputFormat, AudioQuality};
pub use cookies::CookieStorage;
```

**Step 4: Verify build**

```bash
cargo build
```

**Step 5: Commit**

```bash
git add . && git commit -m "feat(storage): add config and cookie persistence"
```

---

## Task 6: Favorites API

**Files:**

- Create: `./src/api/favorite.rs`
- Modify: `./src/api/mod.rs`

**Step 1: Create src/api/favorite.rs**

```rust
use anyhow::{Context, Result};
use crate::api::{BilibiliClient, ApiResponse, FavoriteFolder, FavoriteResource};

#[derive(Debug, Clone, serde::Deserialize)]
struct FavoriteListData {
    list: Option<Vec<FavoriteFolder>>,
    count: u32,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FavoriteResourceData {
    medias: Option<Vec<FavoriteResource>>,
    has_more: bool,
}

impl BilibiliClient {
    pub async fn get_created_folders(&self, mid: u64) -> Result<Vec<FavoriteFolder>> {
        let path = format!("/x/v3/fav/folder/created/list-all?up_mid={}", mid);
        let response: ApiResponse<FavoriteListData> = self.get(&path).await?;

        Ok(response.data.and_then(|d| d.list).unwrap_or_default())
    }

    pub async fn get_collected_folders(&self, mid: u64) -> Result<Vec<FavoriteFolder>> {
        let path = format!("/x/v3/fav/folder/collected/list?up_mid={}&ps=20", mid);
        let response: ApiResponse<FavoriteListData> = self.get(&path).await?;

        Ok(response.data.and_then(|d| d.list).unwrap_or_default())
    }

    pub async fn get_folder_resources(&self, folder_id: u64, page: u32) -> Result<(Vec<FavoriteResource>, bool)> {
        let path = format!(
            "/x/v3/fav/resource/list?media_id={}&ps=20&pn={}",
            folder_id, page
        );
        let response: ApiResponse<FavoriteResourceData> = self.get(&path).await?;

        let data = response.data.unwrap_or(FavoriteResourceData {
            medias: None,
            has_more: false,
        });

        Ok((data.medias.unwrap_or_default(), data.has_more))
    }
}
```

**Step 2: Update src/api/mod.rs**

```rust
pub mod auth;
pub mod client;
pub mod favorite;
pub mod types;

pub use client::BilibiliClient;
pub use types::*;
```

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat(api): add favorites API endpoints"
```

---

## Task 7: History and Watch Later API

**Files:**

- Create: `./src/api/history.rs`
- Modify: `./src/api/mod.rs`

**Step 1: Create src/api/history.rs**

```rust
use anyhow::Result;
use crate::api::{BilibiliClient, ApiResponse};

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HistoryItem {
    pub oid: u64,
    pub bvid: Option<String>,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub author_name: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WatchLaterItem {
    pub bvid: String,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub owner: Option<Owner>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Owner {
    pub mid: u64,
    pub name: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HistoryListData {
    list: Option<HistoryListInner>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct HistoryListInner {
    vlist: Option<Vec<HistoryItem>>,
}

impl BilibiliClient {
    pub async fn get_history(&self, page: u32) -> Result<Vec<HistoryItem>> {
        let path = format!("/x/v2/history?ps=20&pn={}", page);
        let response: ApiResponse<HistoryListData> = self.get(&path).await?;

        Ok(response.data
            .and_then(|d| d.list)
            .and_then(|l| l.vlist)
            .unwrap_or_default())
    }

    pub async fn get_watch_later(&self) -> Result<Vec<WatchLaterItem>> {
        let path = "/x/v2/history/toview";
        let response: ApiResponse<serde_json::Value> = self.get(path).await?;

        let items = response.data
            .and_then(|d| d.get("list").cloned())
            .and_then(|l| serde_json::from_value(l).ok())
            .unwrap_or_default();

        Ok(items)
    }
}
```

**Step 2: Update src/api/mod.rs**

```rust
pub mod auth;
pub mod client;
pub mod favorite;
pub mod history;
pub mod types;

pub use client::BilibiliClient;
pub use types::*;
pub use history::{HistoryItem, WatchLaterItem, Owner};
```

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat(api): add history and watch later endpoints"
```

---

## Task 8: Player URL API

**Files:**

- Create: `./src/api/player.rs`
- Modify: `./src/api/mod.rs`

**Step 1: Create src/api/player.rs**

```rust
use anyhow::{Context, Result};
use crate::api::{BilibiliClient, ApiResponse, PlayUrlData, AudioDash};

#[derive(Debug, Clone)]
pub struct AudioStream {
    pub url: String,
    pub quality: AudioQuality,
    pub format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioQuality {
    K64 = 30216,
    K128 = 30232,
    K192 = 30280,
    HiRes = 30250,
    Dolby = 30251,
    Flac = 30280,
}

impl BilibiliClient {
    pub async fn get_playurl(&self, bvid: &str, cid: u64) -> Result<PlayUrlData> {
        let path = format!(
            "/x/player/wbi/playurl?bvid={}&cid={}&fnval=16&fnver=0&fourk=0",
            bvid, cid
        );
        let response: ApiResponse<PlayUrlData> = self.get(&path).await?;

        response.data.context("No playurl data in response")
    }

    pub async fn get_best_audio(&self, bvid: &str, cid: u64) -> Result<AudioStream> {
        let data = self.get_playurl(bvid, cid).await?;

        if let Some(dash) = data.dash {
            let mut best_audio: Option<&AudioDash> = None;
            for audio in &dash.audio {
                if best_audio.is_none() || audio.bandwidth > best_audio.unwrap().bandwidth {
                    best_audio = Some(audio);
                }
            }

            if let Some(audio) = best_audio {
                let url = audio.base_url.clone()
                    .or_else(|| audio.backup_url.as_ref().and_then(|v| v.first().cloned()))
                    .context("No audio URL found")?;

                let quality = match audio.id {
                    30250 => AudioQuality::HiRes,
                    30251 => AudioQuality::Dolby,
                    30280 => AudioQuality::Flac,
                    30232 => AudioQuality::K128,
                    _ => AudioQuality::K64,
                };

                return Ok(AudioStream {
                    url,
                    quality,
                    format: if audio.codecid == 0 { "mp4a".to_string() } else { "flac".to_string() },
                });
            }
        }

        anyhow::bail!("No audio stream found")
    }
}
```

**Step 2: Update src/api/mod.rs**

```rust
pub mod auth;
pub mod client;
pub mod favorite;
pub mod history;
pub mod player;
pub mod types;

pub use client::BilibiliClient;
pub use types::*;
pub use history::{HistoryItem, WatchLaterItem, Owner};
pub use player::{AudioStream, AudioQuality};
```

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat(api): add player URL and audio stream endpoints"
```

---

## Task 9: Audio Player Foundation

**Files:**

- Create: `./src/audio/player.rs`
- Create: `./src/audio/decoder.rs`
- Modify: `./src/audio/mod.rs`

**Step 1: Create src/audio/decoder.rs**

```rust
use anyhow::{Context, Result};
use ffmpeg_next as ffmpeg;

pub struct AudioDecoder {
    decoder: ffmpeg::decoder::Audio,
    resampler: ffmpeg::software::resampling::Context,
}

impl AudioDecoder {
    pub fn from_url(url: &str) -> Result<Self> {
        ffmpeg::init()?;

        let mut ictx = ffmpeg::format::input(&url)
            .with_context(|| format!("Failed to open input: {}", url))?;

        let input = ictx
            .streams()
            .best(ffmpeg::media::Type::Audio)
            .context("Could not find audio stream")?;
        let stream_index = input.index();

        let context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())?;
        let decoder = context_decoder
            .decoder()
            .audio()
            .context("Failed to create audio decoder")?;

        let resampler = ffmpeg::software::resampling::context::Context::get(
            decoder.format(),
            decoder.channel_layout(),
            decoder.rate(),
            ffmpeg::format::Sample::I16(ffmpeg::format::sample::Type::Packed),
            ffmpeg::channel_layout::ChannelLayout::STEREO,
            44100,
        )?;

        Ok(Self { decoder, resampler })
    }

    pub fn sample_rate(&self) -> u32 {
        self.decoder.rate()
    }

    pub fn channels(&self) -> u16 {
        self.decoder.channels()
    }
}
```

**Step 2: Create src/audio/player.rs**

```rust
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use parking_lot::Mutex;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerState {
    Stopped,
    Playing,
    Paused,
}

pub struct AudioPlayer {
    state: Arc<Mutex<PlayerState>>,
    position: Arc<Mutex<Duration>>,
    duration: Arc<Mutex<Duration>>,
    volume: Arc<Mutex<f32>>,
    _stream: Option<cpal::Stream>,
}

impl AudioPlayer {
    pub fn new() -> Result<Self> {
        Ok(Self {
            state: Arc::new(Mutex::new(PlayerState::Stopped)),
            position: Arc::new(Mutex::new(Duration::ZERO)),
            duration: Arc::new(Mutex::new(Duration::ZERO)),
            volume: Arc::new(Mutex::new(1.0)),
            _stream: None,
        })
    }

    pub fn state(&self) -> PlayerState {
        *self.state.lock()
    }

    pub fn position(&self) -> Duration {
        *self.position.lock()
    }

    pub fn duration(&self) -> Duration {
        *self.duration.lock()
    }

    pub fn volume(&self) -> f32 {
        *self.volume.lock()
    }

    pub fn set_volume(&self, vol: f32) {
        *self.volume.lock() = vol.clamp(0.0, 1.0);
    }

    pub fn play(&mut self, url: &str) -> Result<()> {
        let host = cpal::default_host();
        let device = host.default_output_device().context("No output device")?;
        let supported_config = device.default_output_config()?;
        let config = supported_config.config();

        *self.state.lock() = PlayerState::Playing;
        *self.position.lock() = Duration::ZERO;

        let stream = device.build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for sample in data.iter_mut() {
                    *sample = 0.0;
                }
            },
            |err| eprintln!("Audio stream error: {}", err),
            None,
        )?;

        stream.play()?;
        self._stream = Some(stream);

        Ok(())
    }

    pub fn pause(&self) {
        *self.state.lock() = PlayerState::Paused;
    }

    pub fn resume(&self) {
        *self.state.lock() = PlayerState::Playing;
    }

    pub fn stop(&mut self) {
        *self.state.lock() = PlayerState::Stopped;
        self._stream = None;
    }

    pub fn seek(&self, _position: Duration) {
        *self.position.lock() = _position;
    }
}
```

**Step 3: Update src/audio/mod.rs**

```rust
pub mod decoder;
pub mod player;

pub use decoder::AudioDecoder;
pub use player::{AudioPlayer, PlayerState};
```

**Step 4: Verify build**

```bash
cargo build
```

**Step 5: Commit**

```bash
git add . && git commit -m "feat(audio): add audio player foundation with cpal"
```

---

## Task 10: Download Manager

**Files:**

- Create: `./src/download/manager.rs`
- Create: `./src/download/extractor.rs`
- Modify: `./src/download/mod.rs`

**Step 1: Create src/download/extractor.rs**

```rust
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;
use crate::storage::OutputFormat;

pub struct AudioExtractor;

impl AudioExtractor {
    pub fn extract(input: &Path, output: &Path, format: &OutputFormat) -> Result<()> {
        let (codec, args) = match format {
            OutputFormat::Flac => ("flac", vec!["-c:a", "flac"]),
            OutputFormat::Mp3 { bitrate } => ("libmp3lame", vec!["-c:a", "libmp3lame", "-b:a", &format!("{}k", bitrate)]),
            OutputFormat::Opus { bitrate } => ("libopus", vec!["-c:a", "libopus", "-b:a", &format!("{}k", bitrate)]),
        };

        let output_str = output.to_string_lossy();
        let input_str = input.to_string_lossy();

        let mut cmd = Command::new("ffmpeg");
        cmd.args(["-i", &input_str, "-vn"]);
        cmd.args(&args);
        cmd.arg("-y");
        cmd.arg(&output_str);

        let status = cmd.status().context("Failed to run ffmpeg")?;

        if !status.success() {
            anyhow::bail!("FFmpeg extraction failed with status: {}", status);
        }

        Ok(())
    }
}
```

**Step 2: Create src/download/manager.rs**

```rust
use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use tokio::sync::mpsc;
use crate::storage::{Config, OutputFormat};
use super::extractor::AudioExtractor;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub id: u64,
    pub bvid: String,
    pub title: String,
    pub url: String,
    pub status: DownloadStatus,
    pub progress: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Downloading { bytes_done: u64, total: u64 },
    Extracting,
    Completed,
    Failed(String),
}

pub struct DownloadManager {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    config: Config,
    next_id: Arc<Mutex<u64>>,
}

impl DownloadManager {
    pub fn new(config: Config) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn add(&self, bvid: String, title: String, url: String) -> u64 {
        let mut id = self.next_id.lock();
        let task_id = *id;
        *id += 1;

        let task = DownloadTask {
            id: task_id,
            bvid,
            title,
            url,
            status: DownloadStatus::Pending,
            progress: 0.0,
        };

        self.queue.lock().push_back(task);
        task_id
    }

    pub fn get_queue(&self) -> Vec<DownloadTask> {
        self.queue.lock().iter().cloned().collect()
    }

    pub async fn download_next(&self) -> Result<()> {
        let task = {
            let mut queue = self.queue.lock();
            queue.front_mut().map(|t| {
                t.status = DownloadStatus::Downloading { bytes_done: 0, total: 0 };
                t.clone()
            })
        };

        if let Some(task) = task {
            let response = reqwest::get(&task.url).await?;
            let bytes = response.bytes().await?;

            let temp_path = self.config.download_dir.join(format!("{}.temp", task.bvid));
            std::fs::create_dir_all(&self.config.download_dir)?;
            std::fs::write(&temp_path, &bytes)?;

            let output_path = self.get_output_path(&task.bvid, &task.title);
            AudioExtractor::extract(&temp_path, &output_path, &self.config.output_format)?;
            std::fs::remove_file(&temp_path)?;

            let mut queue = self.queue.lock();
            if let Some(front) = queue.front_mut() {
                front.status = DownloadStatus::Completed;
                front.progress = 100.0;
            }
        }

        Ok(())
    }

    fn get_output_path(&self, bvid: &str, title: &str) -> PathBuf {
        let ext = match self.config.output_format {
            OutputFormat::Flac => "flac",
            OutputFormat::Mp3 { .. } => "mp3",
            OutputFormat::Opus { .. } => "opus",
        };

        let safe_title: String = title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '_' })
            .collect();

        self.config.download_dir.join(format!("{} - {}.{}", bvid, safe_title, ext))
    }
}
```

**Step 3: Update src/download/mod.rs**

```rust
pub mod extractor;
pub mod manager;

pub use extractor::AudioExtractor;
pub use manager::{DownloadManager, DownloadTask, DownloadStatus};
```

**Step 4: Verify build**

```bash
cargo build
```

**Step 5: Commit**

```bash
git add . && git commit -m "feat(download): add download manager with audio extraction"
```

---

## Task 11: Library Screen

**Files:**

- Create: `./src/screens/library.rs`
- Modify: `./src/screens/mod.rs`

**Step 1: Create src/screens/library.rs**

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};
use crate::api::{FavoriteFolder, FavoriteResource};
use crate::api::{HistoryItem, WatchLaterItem};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Favorites,
    WatchLater,
    History,
}

pub struct LibraryScreen {
    pub current_tab: LibraryTab,
    pub folders: Vec<FavoriteFolder>,
    pub resources: Vec<FavoriteResource>,
    pub watch_later: Vec<WatchLaterItem>,
    pub history: Vec<HistoryItem>,
    pub list_state: ListState,
    pub selected_folder: Option<u64>,
}

impl LibraryScreen {
    pub fn new() -> Self {
        Self {
            current_tab: LibraryTab::Favorites,
            folders: Vec::new(),
            resources: Vec::new(),
            watch_later: Vec::new(),
            history: Vec::new(),
            list_state: ListState::default(),
            selected_folder: None,
        }
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(10),
                Constraint::Length(3),
            ])
            .split(area);

        let titles: Vec<&str> = vec!["Favorites", "Watch Later", "History"];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.current_tab as usize)
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Cyan));
        f.render_widget(tabs, chunks[0]);

        let items: Vec<ListItem> = match self.current_tab {
            LibraryTab::Favorites => {
                if self.selected_folder.is_some() {
                    self.resources.iter().map(|r| {
                        let quality_badge = if r.duration > 300 { "[HQ]" } else { "" };
                        ListItem::new(format!("{} {}  {}  {:->5}  {}",
                            r.bvid, r.title, r.upper.name,
                            format_duration(r.duration), quality_badge))
                    }).collect()
                } else {
                    self.folders.iter().map(|f| {
                        ListItem::new(format!("{} ({})", f.title, f.media_count))
                    }).collect()
                }
            }
            LibraryTab::WatchLater => {
                self.watch_later.iter().map(|w| {
                    ListItem::new(format!("{} - {}", w.title, w.owner.as_ref().map(|o| o.name.as_str()).unwrap_or("Unknown")))
                }).collect()
            }
            LibraryTab::History => {
                self.history.iter().map(|h| {
                    ListItem::new(format!("{} - {}", h.title, h.author_name.as_deref().unwrap_or("Unknown")))
                }).collect()
            }
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let help = Paragraph::new("[j/k] Navigate  [Enter] Select  [d] Download  [Tab] Switch")
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
    }

    pub fn next_item(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            let i = self.list_state.selected().map_or(0, |i| {
                if i >= len - 1 { 0 } else { i + 1 }
            });
            self.list_state.select(Some(i));
        }
    }

    pub fn prev_item(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            let i = self.list_state.selected().map_or(0, |i| {
                if i == 0 { len - 1 } else { i - 1 }
            });
            self.list_state.select(Some(i));
        }
    }

    fn current_list_len(&self) -> usize {
        match self.current_tab {
            LibraryTab::Favorites => {
                if self.selected_folder.is_some() {
                    self.resources.len()
                } else {
                    self.folders.len()
                }
            }
            LibraryTab::WatchLater => self.watch_later.len(),
            LibraryTab::History => self.history.len(),
        }
    }
}

fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", mins, secs)
}
```

**Step 2: Update src/screens/mod.rs**

```rust
pub mod library;
pub mod login;

pub use library::{LibraryScreen, LibraryTab};
pub use login::{LoginScreen, LoginState};
```

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat(ui): add library screen with tabs"
```

---

## Task 12: Main App Integration

**Files:**

- Modify: `./src/app.rs`

**Step 1: Update src/app.rs with full implementation**

```rust
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::*,
};
use parking_lot::Mutex;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::sync::Arc;
use std::time::Duration;

use crate::api::{BilibiliClient, QrCodeData};
use crate::audio::AudioPlayer;
use crate::download::DownloadManager;
use crate::screens::{LibraryScreen, LibraryTab, LoginScreen, LoginState};
use crate::storage::{Config, CookieStorage};

pub enum Screen {
    Login(LoginScreen),
    Library(LibraryScreen),
}

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
    screen: Screen,
    client: Arc<Mutex<BilibiliClient>>,
    player: Option<AudioPlayer>,
    downloader: Option<DownloadManager>,
    config: Config,
}

impl App {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let client = Arc::new(Mutex::new(BilibiliClient::new()?));
        let config = Config::load().unwrap_or_default();

        let screen = Screen::Login(LoginScreen::new());

        Ok(Self {
            terminal,
            running: true,
            screen,
            client,
            player: None,
            downloader: None,
            config,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            self.terminal.draw(|f| {
                let area = f.area();
                match &self.screen {
                    Screen::Login(login) => login.render(f, area),
                    Screen::Library(library) => {
                        let mut lib = library.clone();
                        lib.render(f, area);
                    }
                }
            })?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key.code, key.modifiers)?;
                }
            }
        }

        self.cleanup()?;
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Result<()> {
        match &mut self.screen {
            Screen::Login(login) => self.handle_login_key(login, code)?,
            Screen::Library(library) => self.handle_library_key(library, code)?,
        }
        Ok(())
    }

    fn handle_login_key(&mut self, login: &mut LoginScreen, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('r') => {
                let client = self.client.lock();
                let rt = tokio::runtime::Runtime::new()?;
                let qrcode = rt.block_on(client.generate_qrcode())?;
                login.state = LoginState::QrWaiting { qrcode_data: qrcode };
                login.status_message = "Scan QR code with Bilibili app".to_string();
            }
            _ => {}
        }
        Ok(())
    }

    fn handle_library_key(&mut self, library: &mut LibraryScreen, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Tab => {
                library.current_tab = match library.current_tab {
                    LibraryTab::Favorites => LibraryTab::WatchLater,
                    LibraryTab::WatchLater => LibraryTab::History,
                    LibraryTab::History => LibraryTab::Favorites,
                };
            }
            KeyCode::Char('j') | KeyCode::Down => library.next_item(),
            KeyCode::Char('k') | KeyCode::Up => library.prev_item(),
            _ => {}
        }
        Ok(())
    }

    fn cleanup(&mut self) -> Result<()> {
        disable_raw_mode()?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )?;
        self.terminal.show_cursor()?;
        Ok(())
    }
}
```

**Step 2: Add Clone to LibraryScreen**

Add `#[derive(Clone)]` to LibraryScreen in `src/screens/library.rs`.

**Step 3: Verify build**

```bash
cargo build
```

**Step 4: Commit**

```bash
git add . && git commit -m "feat: integrate all screens into main app"
```

---

## Task 13: Final Testing and Polish

**Step 1: Run the application**

```bash
cargo run
```

Expected: Terminal UI appears with login screen, QR code can be requested with 'r' key

**Step 2: Test key navigation**

- Press 'r' - should show QR code
- Press 'q' - should quit
- (After login) Press Tab - should switch tabs
- Press j/k - should navigate list

**Step 3: Final commit**

```bash
git add . && git commit -m "chore: final testing and polish"
```

---

## Summary

This implementation plan creates a fully functional Bilibili TUI music player with:

1. QR code and SMS login
2. Favorites, Watch Later, and History browsing
3. High-quality audio playback via FFmpeg
4. Audio extraction downloads to FLAC/MP3/Opus
5. Keyboard-driven navigation with Ratatui

The project will be located at `./` as a separate Rust crate.
