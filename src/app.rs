use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::*,
};
use parking_lot::Mutex;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::api::BilibiliClient;
use crate::audio::{AudioPlayer, PlayerState};
use crate::download::DownloadManager;
use crate::playing_list::PlayingListManager;
use crate::screens::{
    LibraryScreen, LibraryTab, LoginScreen, LoginState, NextAction, SettingsScreen,
};
use crate::storage::{Config, CookieStorage, Settings};

pub enum Screen {
    Login(LoginScreen),
    Library(LibraryScreen),
    Settings(SettingsScreen),
}

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
    screen: Screen,
    client: Arc<Mutex<BilibiliClient>>,
    player: Option<AudioPlayer>,
    downloader: Option<DownloadManager>,
    config: Config,
    last_qr_poll: Option<Instant>,
    settings: Settings,
    playing_list: Arc<Mutex<PlayingListManager>>,
    previous_library: Option<LibraryScreen>,
    previous_player_state: Option<PlayerState>,
}

impl App {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = std::io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        let mut client = BilibiliClient::new()?;
        let config = Config::load().unwrap_or_default();
        let settings = Settings::load().unwrap_or_default();
        let playing_list = Arc::new(Mutex::new(PlayingListManager::new().unwrap_or_else(|e| {
            eprintln!("Failed to load playing list: {}", e);
            PlayingListManager::new_empty().unwrap()
        })));

        let has_session = Self::try_restore_session(&mut client).unwrap_or(false);

        let client = Arc::new(Mutex::new(client));

        let screen = if has_session {
            let mut library = LibraryScreen::new();
            library.set_loop_mode(settings.loop_mode);
            let rt = tokio::runtime::Runtime::new()?;
            if let Err(e) = rt.block_on(library.load_data(client.clone())) {
                eprintln!("Failed to load library data: {}", e);
                Screen::Login(LoginScreen::new())
            } else {
                Screen::Library(library)
            }
        } else {
            Screen::Login(LoginScreen::new())
        };

        Ok(Self {
            terminal,
            running: true,
            screen,
            client,
            player: None,
            downloader: None,
            config,
            last_qr_poll: None,
            settings,
            playing_list,
            previous_library: None,
            previous_player_state: None,
        })
    }

    fn try_restore_session(client: &mut BilibiliClient) -> Result<bool> {
        let cookies = match CookieStorage::load()? {
            Some(c) => c,
            None => return Ok(false),
        };

        if cookies.is_empty() {
            return Ok(false);
        }

        client.load_cookies(&cookies)?;

        let rt = tokio::runtime::Runtime::new()?;
        match rt.block_on(client.get_user_info()) {
            Ok(_) => Ok(true),
            Err(_) => {
                let _ = CookieStorage::clear();
                Ok(false)
            }
        }
    }

    pub fn run(&mut self) -> Result<()> {
        while self.running {
            self.terminal.draw(|f| {
                let area = f.area();
                match &self.screen {
                    Screen::Login(login) => login.render(f, area),
                    Screen::Library(library) => {
                        let mut lib = library.clone();
                        lib.render(f, area, self.player.as_ref(), self.playing_list.clone());
                    }
                    Screen::Settings(settings_screen) => {
                        let mut s = settings_screen.clone();
                        s.render(f, area);
                    }
                }
            })?;

            if event::poll(Duration::from_millis(100))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key.code, key.modifiers)?;
                }
            }

            self.poll_qr_login()?;
            self.poll_playback_completion()?;
        }

        self.cleanup()?;
        Ok(())
    }

    fn handle_key(&mut self, code: KeyCode, _modifiers: KeyModifiers) -> Result<()> {
        match &mut self.screen {
            Screen::Login(login) => match code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('r') => {
                    let client = self.client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    let qrcode = rt.block_on(client.generate_qrcode())?;
                    login.state = LoginState::QrWaiting {
                        qrcode_data: qrcode,
                    };
                    login.status_message = "Scan QR code with Bilibili app".to_string();
                }
                _ => {}
            },
            Screen::Library(library) => match code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Tab => {
                    library.current_tab = match library.current_tab {
                        LibraryTab::Favorites => LibraryTab::WatchLater,
                        LibraryTab::WatchLater => LibraryTab::History,
                        LibraryTab::History => LibraryTab::PlayingList,
                        LibraryTab::PlayingList => LibraryTab::Favorites,
                    };
                }
                KeyCode::Char('j') | KeyCode::Down => library.next_item(),
                KeyCode::Char('k') | KeyCode::Up => library.prev_item(),
                KeyCode::Enter => {
                    if let Err(e) = library.handle_enter(self.client.clone(), &mut self.player) {
                        eprintln!("Failed to handle enter: {}", e);
                    }
                    self.apply_volume();
                }
                KeyCode::Esc | KeyCode::Backspace => library.go_back(),
                KeyCode::Char('s') => {
                    self.previous_library = Some(library.clone());
                    let settings_screen = SettingsScreen::new(self.settings.clone());
                    self.screen = Screen::Settings(settings_screen);
                }
                _ => {}
            },
            Screen::Settings(settings_screen) => match code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('s') | KeyCode::Esc => {
                    if let Some(mut library) = self.previous_library.take() {
                        library.set_loop_mode(self.settings.loop_mode);
                        self.screen = Screen::Library(library);
                    } else {
                        let mut library = LibraryScreen::new();
                        library.set_loop_mode(self.settings.loop_mode);
                        if let Err(e) = self.load_library_data_into(&mut library) {
                            eprintln!("Failed to load library data: {}", e);
                        }
                        self.screen = Screen::Library(library);
                    }
                }
                KeyCode::Char('j') | KeyCode::Down => settings_screen.next_item(),
                KeyCode::Char('k') | KeyCode::Up => settings_screen.prev_item(),
                KeyCode::Char('l') | KeyCode::Right => {
                    settings_screen.adjust_up();
                    self.settings = settings_screen.settings.clone();
                    self.apply_volume();
                }
                KeyCode::Char('h') | KeyCode::Left => {
                    settings_screen.adjust_down();
                    self.settings = settings_screen.settings.clone();
                    self.apply_volume();
                }
                _ => {}
            },
        }
        Ok(())
    }

    fn poll_qr_login(&mut self) -> Result<()> {
        if let Screen::Login(login) = &mut self.screen {
            let qrcode_key = match &login.state {
                LoginState::QrWaiting { qrcode_data } | LoginState::QrScanned { qrcode_data } => {
                    Some(qrcode_data.qrcode_key.clone())
                }
                _ => None,
            };

            if let Some(qrcode_key) = qrcode_key {
                let should_poll = self
                    .last_qr_poll
                    .map(|last| last.elapsed() >= Duration::from_secs(2))
                    .unwrap_or(true);

                if should_poll {
                    self.last_qr_poll = Some(Instant::now());

                    let poll_result = {
                        let client = self.client.lock();
                        let rt = tokio::runtime::Runtime::new()?;
                        rt.block_on(client.poll_qrcode(&qrcode_key))
                    };

                    match poll_result {
                        Ok(poll_data) => match poll_data.code {
                            0 => {
                                if let Some(url) = poll_data.url {
                                    match self.handle_qr_login_success(&url) {
                                        Ok(_) => {
                                            let mut library = LibraryScreen::new();
                                            library.set_loop_mode(self.settings.loop_mode);
                                            self.screen = Screen::Library(library);
                                            if let Err(e) = self.load_library_data() {
                                                eprintln!("Failed to load library data: {}", e);
                                            }
                                        }
                                        Err(e) => {
                                            if let Screen::Login(login) = &mut self.screen {
                                                login.state = LoginState::Error(format!(
                                                    "Login failed: {}",
                                                    e
                                                ));
                                            }
                                        }
                                    }
                                }
                            }
                            86038 => {
                                login.state = LoginState::Error(
                                    "QR code expired. Press 'r' to refresh.".to_string(),
                                );
                            }
                            86090 => {
                                if let LoginState::QrWaiting { qrcode_data } = &login.state {
                                    login.state = LoginState::QrScanned {
                                        qrcode_data: qrcode_data.clone(),
                                    };
                                    login.status_message =
                                        "QR code scanned! Please confirm on your device..."
                                            .to_string();
                                }
                            }
                            _ => {}
                        },
                        Err(_) => {}
                    }
                }
            }
        }
        Ok(())
    }

    fn poll_playback_completion(&mut self) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            let current_state = self.player.as_ref().map(|p| p.state());

            let was_playing = self.previous_player_state == Some(PlayerState::Playing);
            let now_stopped = current_state == Some(PlayerState::Stopped);

            if was_playing && now_stopped {
                if let Some(next_action) = library.get_next_action() {
                    match next_action {
                        NextAction::ReplayCurrent => {
                            self.replay_current()?;
                        }
                        NextAction::PlayNext(idx) => {
                            library.list_state.select(Some(idx));
                            if let Err(e) =
                                library.handle_enter(self.client.clone(), &mut self.player)
                            {
                                eprintln!("Failed to play next: {}", e);
                            }
                            self.apply_volume();
                        }
                    }
                }
            }

            self.previous_player_state = current_state;
        }
        Ok(())
    }

    fn replay_current(&mut self) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            if let Err(e) = library.handle_enter(self.client.clone(), &mut self.player) {
                eprintln!("Failed to replay: {}", e);
            }
            self.apply_volume();
        }
        Ok(())
    }

    fn handle_qr_login_success(&mut self, url: &str) -> Result<()> {
        let cookies = if url.contains('?') {
            url.split('?')
                .nth(1)
                .unwrap_or("")
                .split('&')
                .filter(|s| {
                    s.starts_with("DedeUserID=")
                        || s.starts_with("SESSDATA=")
                        || s.starts_with("bili_jct=")
                        || s.starts_with("DedeUserID__ckMd5=")
                })
                .collect::<Vec<_>>()
                .join("; ")
        } else {
            url.split('&')
                .filter(|s| {
                    s.starts_with("DedeUserID=")
                        || s.starts_with("SESSDATA=")
                        || s.starts_with("bili_jct=")
                        || s.starts_with("DedeUserID__ckMd5=")
                })
                .collect::<Vec<_>>()
                .join("; ")
        };

        if cookies.is_empty() {
            anyhow::bail!("No cookies found in login URL");
        }

        crate::storage::CookieStorage::save(&cookies)?;

        let mut client = self.client.lock();

        if let Some(csrf) = cookies
            .split(';')
            .find(|s| s.trim().starts_with("bili_jct="))
            .and_then(|s| s.split('=').nth(1))
            .map(|s| s.trim().to_string())
        {
            client.set_csrf(csrf);
        }

        if let Some(mid) = cookies
            .split(';')
            .find(|s| s.trim().starts_with("DedeUserID="))
            .and_then(|s| s.split('=').nth(1))
            .and_then(|s| s.trim().parse::<u64>().ok())
        {
            client.set_mid(mid);
        }

        Ok(())
    }

    fn load_library_data(&mut self) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(library.load_data(self.client.clone()))?;
        }
        Ok(())
    }

    fn load_library_data_into(&mut self, library: &mut LibraryScreen) -> Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        rt.block_on(library.load_data(self.client.clone()))?;
        Ok(())
    }

    fn apply_volume(&mut self) {
        if let Some(player) = &self.player {
            player.set_volume(self.settings.volume_float());
        }
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
