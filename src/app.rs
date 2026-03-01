use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::*,
};
use parking_lot::Mutex;
use ratatui::{backend::CrosstermBackend, Terminal};
use std::sync::Arc;
use std::time::Duration;

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
