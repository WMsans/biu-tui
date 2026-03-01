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
use crate::audio::AudioPlayer;
use crate::download::DownloadManager;
use crate::screens::{LibraryScreen, LibraryTab, LoginScreen, LoginState};
use crate::storage::Config;

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
    last_qr_poll: Option<Instant>,
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
            last_qr_poll: None,
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

            self.poll_qr_login()?;
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
                        LibraryTab::History => LibraryTab::Favorites,
                    };
                }
                KeyCode::Char('j') | KeyCode::Down => library.next_item(),
                KeyCode::Char('k') | KeyCode::Up => library.prev_item(),
                _ => {}
            },
        }
        Ok(())
    }

    fn poll_qr_login(&mut self) -> Result<()> {
        if let Screen::Login(login) = &mut self.screen {
            let qrcode_key = match &login.state {
                LoginState::QrWaiting { qrcode_data } | LoginState::QrScanned { qrcode_data } => {
                    eprintln!(
                        "State matches, will poll with key: {}",
                        qrcode_data.qrcode_key
                    );
                    Some(qrcode_data.qrcode_key.clone())
                }
                _ => {
                    eprintln!(
                        "State doesn't match for polling: {:?}",
                        std::mem::discriminant(&login.state)
                    );
                    None
                }
            };

            if let Some(qrcode_key) = qrcode_key {
                let should_poll = self
                    .last_qr_poll
                    .map(|last| last.elapsed() >= Duration::from_secs(2))
                    .unwrap_or(true);

                eprintln!("Should poll: {}", should_poll);

                if should_poll {
                    self.last_qr_poll = Some(Instant::now());

                    let poll_result = {
                        let client = self.client.lock();
                        let rt = tokio::runtime::Runtime::new()?;
                        rt.block_on(client.poll_qrcode(&qrcode_key))
                    };

                    match poll_result {
                        Ok(poll_data) => {
                            eprintln!(
                                "Poll result: code={}, message={}, url={:?}",
                                poll_data.code, poll_data.message, poll_data.url
                            );
                            match poll_data.code {
                                0 => {
                                    if let Some(url) = poll_data.url {
                                        eprintln!("Login successful, URL: {}", url);
                                        match self.handle_qr_login_success(&url) {
                                            Ok(_) => {
                                                if let Screen::Login(login) = &mut self.screen {
                                                    login.state = LoginState::LoggedIn;
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
                                    eprintln!("Got code 86090 (scanned, waiting confirmation)");
                                    if let LoginState::QrWaiting { qrcode_data } = &login.state {
                                        eprintln!("Transitioning QrWaiting -> QrScanned");
                                        login.state = LoginState::QrScanned {
                                            qrcode_data: qrcode_data.clone(),
                                        };
                                        login.status_message =
                                            "QR code scanned! Please confirm on your device..."
                                                .to_string();
                                    } else {
                                        eprintln!("Already in QrScanned state, continuing to wait");
                                    }
                                }
                                _ => {
                                    eprintln!("Unhandled poll code: {}", poll_data.code);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Poll error: {:?}", e);
                        }
                    }
                }
            }
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

        if let Some(csrf) = cookies
            .split(';')
            .find(|s| s.trim().starts_with("bili_jct="))
            .and_then(|s| s.split('=').nth(1))
            .map(|s| s.trim().to_string())
        {
            let mut client = self.client.lock();
            client.set_csrf(csrf);
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
