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
use crate::api::FavoriteFolder;
use crate::audio::{AudioPlayer, PlayerState};
use crate::download::DownloadManager;
use crate::mpris::{MprisCommand, MprisManager};
use crate::playing_list::{PlayingListManager, PlaylistItem};
use crate::screens::library::NavigationLevel;
use crate::screens::{
    LibraryScreen, LibraryTab, LoginScreen, LoginState, Searchable, SettingsScreen,
};
use crate::storage::{Config, CookieStorage, LoopMode, Settings};

pub enum Screen {
    Login(LoginScreen),
    Library(Box<LibraryScreen>),
    Settings(SettingsScreen),
}

pub struct App {
    terminal: Terminal<CrosstermBackend<std::io::Stdout>>,
    running: bool,
    screen: Screen,
    client: Arc<Mutex<BilibiliClient>>,
    player: Option<AudioPlayer>,
    _downloader: Option<DownloadManager>,
    _config: Config,
    last_qr_poll: Option<Instant>,
    settings: Settings,
    playing_list: Arc<Mutex<PlayingListManager>>,
    previous_library: Option<LibraryScreen>,
    previous_player_state: Option<PlayerState>,
    mpris: Option<MprisManager>,
}

impl App {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;

        let mut stdout = std::io::stdout();
        if let Err(e) = execute!(stdout, EnterAlternateScreen, EnableMouseCapture) {
            let _ = disable_raw_mode();
            return Err(e.into());
        }

        let backend = CrosstermBackend::new(stdout);
        let terminal = match Terminal::new(backend) {
            Ok(t) => t,
            Err(e) => {
                let _ = disable_raw_mode();
                let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
                return Err(e.into());
            }
        };

        let result = Self::init_app_state(terminal);

        match result {
            Ok(app) => Ok(app),
            Err(e) => {
                let _ = disable_raw_mode();
                let _ = execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
                Err(e)
            }
        }
    }

    fn init_app_state(terminal: Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<Self> {
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
            if let Err(e) = library.load_data(client.clone()) {
                eprintln!("Failed to load library data: {}", e);
                Screen::Login(LoginScreen::new())
            } else {
                Screen::Library(Box::new(library))
            }
        } else {
            Screen::Login(LoginScreen::new())
        };

        let mpris = MprisManager::new()
            .map_err(|e| eprintln!("MPRIS initialization failed: {}", e))
            .ok();

        Ok(Self {
            terminal,
            running: true,
            screen,
            client,
            player: None,
            _downloader: None,
            _config: config,
            last_qr_poll: None,
            settings,
            playing_list,
            previous_library: None,
            previous_player_state: None,
            mpris,
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
                        let mut lib = (*library).clone();
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
                    if key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        self.running = false;
                        continue;
                    }
                    self.handle_key(key.code, key.modifiers)?;
                }
            }

            if let (Some(player), Some(mpris)) = (&self.player, &self.mpris) {
                mpris.set_position(player.position());
            }

            if let Some(mpris) = &self.mpris {
                for cmd in mpris.poll_commands() {
                    if let Err(e) = self.handle_mpris_command(cmd) {
                        eprintln!("Failed to handle MPRIS command: {}", e);
                    }
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
            Screen::Library(library) => {
                // Clear status message on any keypress
                library.status_message = None;

                // Handle search input if in search mode
                if let Some(ref mut search_state) = library.search_state {
                    match code {
                        KeyCode::Esc => {
                            self.exit_search_mode(true)?;
                            return Ok(());
                        }
                        KeyCode::Enter => {
                            self.perform_search()?;
                            self.exit_search_mode(false)?;
                            return Ok(());
                        }
                        KeyCode::Char('u') if _modifiers.contains(KeyModifiers::CONTROL) => {
                            search_state.clear();
                            return Ok(());
                        }
                        KeyCode::Backspace => {
                            search_state.pop_char();
                            return Ok(());
                        }
                        KeyCode::Char(c) => {
                            search_state.push_char(c);
                            return Ok(());
                        }
                        _ => {}
                    }
                }

                match code {
                    KeyCode::Char('/') => {
                        if library.search_state.is_none() {
                            self.enter_search_mode()?;
                        }
                    }
                    KeyCode::Char('q') => self.running = false,
                    KeyCode::Tab => {
                        if library.search_state.is_some() {
                            if let Some(original) = library.original_folders.take() {
                                library.folders = original;
                            }
                            if let Some(original) = library.original_resources.take() {
                                library.resources = original;
                            }
                            if let Some(original) = library.original_episodes.take() {
                                library.episodes = original;
                            }
                            if let Some(original) = library.original_watch_later.take() {
                                library.watch_later = original;
                            }
                            if let Some(original) = library.original_history.take() {
                                library.history = original;
                            }
                            library.search_state = None;
                            library.original_folders = None;
                            library.original_resources = None;
                            library.original_episodes = None;
                            library.original_watch_later = None;
                            library.original_history = None;
                        }
                        library.current_tab = match library.current_tab {
                            LibraryTab::Favorites => LibraryTab::WatchLater,
                            LibraryTab::WatchLater => LibraryTab::History,
                            LibraryTab::History => LibraryTab::PlayingList,
                            LibraryTab::PlayingList => LibraryTab::Favorites,
                        };
                        library.reset_selection_for_tab(self.playing_list.clone());
                    }
                    KeyCode::Char('j') | KeyCode::Down => {
                        library.next_item(&self.playing_list, self.client.clone());
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        library.prev_item(&self.playing_list);
                    }
                    KeyCode::Enter => {
                        let old_now_playing = library.now_playing.clone();
                        if let Err(e) = library.handle_enter(
                            self.client.clone(),
                            &mut self.player,
                            self.playing_list.clone(),
                            self.settings.playback_speed,
                        ) {
                            eprintln!("Failed to handle enter: {}", e);
                        }
                        // Extract now_playing info before releasing library borrow
                        let new_now_playing = library.now_playing.clone();
                        self.apply_volume();
                        self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                    }
                    KeyCode::Esc | KeyCode::Backspace => library.go_back(),
                    KeyCode::Char('a') => {
                        if let Err(e) = library
                            .add_to_playing_list(self.playing_list.clone(), self.client.clone())
                        {
                            library.status_message =
                                Some(format!("Failed to add to playing list: {}", e));
                        }
                    }
                    KeyCode::Char('A') => {
                        if let Err(e) = library
                            .add_all_to_playing_list(self.playing_list.clone(), self.client.clone())
                        {
                            library.status_message =
                                Some(format!("Failed to add all to playing list: {}", e));
                        }
                    }
                    KeyCode::Char('d') => {
                        let old_now_playing = library.now_playing.clone();
                        if let Err(e) = library.handle_remove_song(
                            self.playing_list.clone(),
                            self.client.clone(),
                            &mut self.player,
                            self.settings.playback_speed,
                        ) {
                            eprintln!("Failed to remove song: {}", e);
                        }
                        let new_now_playing = library.now_playing.clone();
                        self.notify_mpris_if_playback_changed(&old_now_playing, &new_now_playing);
                    }
                    KeyCode::Char('s') => {
                        self.previous_library = Some((**library).clone());
                        let settings_screen = SettingsScreen::new(self.settings.clone());
                        self.screen = Screen::Settings(settings_screen);
                    }
                    KeyCode::Char(' ') => {
                        self.toggle_playback()?;
                    }
                    KeyCode::Char('h') | KeyCode::Left => {
                        self.handle_seek_backward()?;
                    }
                    KeyCode::Char('l') | KeyCode::Right => {
                        self.handle_seek_forward()?;
                    }
                    _ => {}
                }
            }
            Screen::Settings(settings_screen) => match code {
                KeyCode::Char('q') => self.running = false,
                KeyCode::Char('s') | KeyCode::Esc => {
                    if let Some(mut library) = self.previous_library.take() {
                        library.set_loop_mode(self.settings.loop_mode);
                        self.screen = Screen::Library(Box::new(library));
                    } else {
                        let mut library = LibraryScreen::new();
                        library.set_loop_mode(self.settings.loop_mode);
                        if let Err(e) = self.load_library_data_into(&mut library) {
                            eprintln!("Failed to load library data: {}", e);
                        }
                        self.screen = Screen::Library(Box::new(library));
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

    fn enter_search_mode(&mut self) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            library.search_state = Some(crate::screens::SearchState::new());

            library.original_folders = Some(library.folders.clone());
            library.original_resources = Some(library.resources.clone());
            library.original_episodes = Some(library.episodes.clone());
            library.original_watch_later = Some(library.watch_later.clone());
            library.original_history = Some(library.history.clone());
        }
        Ok(())
    }

    fn exit_search_mode(&mut self, restore: bool) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            if restore {
                if let Some(original) = library.original_folders.take() {
                    library.folders = original;
                }
                if let Some(original) = library.original_resources.take() {
                    library.resources = original;
                }
                if let Some(original) = library.original_episodes.take() {
                    library.episodes = original;
                }
                if let Some(original) = library.original_watch_later.take() {
                    library.watch_later = original;
                }
                if let Some(original) = library.original_history.take() {
                    library.history = original;
                }
            }

            library.search_state = None;
            library.original_folders = None;
            library.original_resources = None;
            library.original_episodes = None;
            library.original_watch_later = None;
            library.original_history = None;
        }
        Ok(())
    }

    fn perform_search(&mut self) -> Result<()> {
        if let Screen::Library(library) = &mut self.screen {
            if let Some(ref search_state) = library.search_state {
                let query = search_state.query.trim();

                if query.is_empty() {
                    if let Some(original) = &library.original_folders {
                        library.folders = original.clone();
                    }
                    if let Some(original) = &library.original_resources {
                        library.resources = original.clone();
                    }
                    if let Some(original) = &library.original_episodes {
                        library.episodes = original.clone();
                    }
                    if let Some(original) = &library.original_watch_later {
                        library.watch_later = original.clone();
                    }
                    if let Some(original) = &library.original_history {
                        library.history = original.clone();
                    }
                    return Ok(());
                }

                match library.current_tab {
                    LibraryTab::Favorites => match &library.nav_level {
                        NavigationLevel::Folders => {
                            if let Some(original) = &library.original_folders {
                                library.folders = original
                                    .iter()
                                    .filter(|f| f.matches(query))
                                    .cloned()
                                    .collect();
                            }
                        }
                        NavigationLevel::Videos { folder_id, .. } => {
                            let folder_id = *folder_id;
                            let client = self.client.clone();
                            let keyword = query.to_string();

                            let results = {
                                let c = client.lock();
                                let rt = tokio::runtime::Runtime::new()?;
                                rt.block_on(c.search_folder_resources(folder_id, &keyword))
                            };

                            match results {
                                Ok(resources) => {
                                    library.resources = resources;
                                }
                                Err(e) => {
                                    library.status_message = Some(format!("Search failed: {}", e));
                                }
                            }
                        }
                        NavigationLevel::Episodes { .. } => {
                            if let Some(original) = &library.original_episodes {
                                library.episodes = original
                                    .iter()
                                    .filter(|ep| ep.matches(query))
                                    .cloned()
                                    .collect();
                            }
                        }
                    },
                    LibraryTab::WatchLater => {
                        let client = self.client.clone();
                        let keyword = query.to_string();

                        let results = {
                            let c = client.lock();
                            let rt = tokio::runtime::Runtime::new()?;
                            rt.block_on(c.search_watch_later(&keyword))
                        };

                        match results {
                            Ok(items) => {
                                library.watch_later = items;
                            }
                            Err(e) => {
                                library.status_message = Some(format!("Search failed: {}", e));
                            }
                        }
                    }
                    LibraryTab::History => {
                        let client = self.client.clone();
                        let keyword = query.to_string();

                        let results = {
                            let c = client.lock();
                            let rt = tokio::runtime::Runtime::new()?;
                            rt.block_on(c.search_history(&keyword))
                        };

                        match results {
                            Ok(items) => {
                                library.history = items;
                            }
                            Err(e) => {
                                library.status_message = Some(format!("Search failed: {}", e));
                            }
                        }
                    }
                    LibraryTab::PlayingList => {
                        let playing_list = self.playing_list.lock();
                        let items = playing_list.items();
                        library.folders = items
                            .iter()
                            .filter(|item| item.matches(query))
                            .map(|item| FavoriteFolder {
                                id: 0,
                                title: format!("{} - {}", item.title, item.artist),
                                media_count: 1,
                            })
                            .collect();
                    }
                }

                library.list_state.select(Some(0));
            }
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

                    if let Ok(poll_data) = poll_result {
                        match poll_data.code {
                            0 => {
                                if let Some(url) = poll_data.url {
                                    match self.handle_qr_login_success(&url) {
                                        Ok(_) => {
                                            let mut library = LibraryScreen::new();
                                            library.set_loop_mode(self.settings.loop_mode);
                                            self.screen = Screen::Library(Box::new(library));
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
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn poll_playback_completion(&mut self) -> Result<()> {
        if let Screen::Library(_library) = &mut self.screen {
            let current_state = self.player.as_ref().map(|p| p.state());

            let was_playing = self.previous_player_state == Some(PlayerState::Playing);
            let now_stopped = current_state == Some(PlayerState::Stopped);

            if was_playing && now_stopped {
                let next_item = match self.settings.loop_mode {
                    LoopMode::LoopOne => self.playing_list.lock().current().cloned(),
                    LoopMode::NoLoop => self.playing_list.lock().advance_to_next().cloned(),
                    LoopMode::LoopList => {
                        let current = self.playing_list.lock().current_index();
                        let count = self.playing_list.lock().items().len();

                        if count == 0 {
                            None
                        } else {
                            let next_idx = match current {
                                Some(idx) if idx + 1 < count => idx + 1,
                                _ => 0,
                            };

                            self.playing_list.lock().jump_to(next_idx);
                            self.playing_list.lock().current().cloned()
                        }
                    }
                };

                if let Some(item) = next_item {
                    self.play_playlist_item(&item)?;
                }
            }

            self.previous_player_state = current_state;
        }
        Ok(())
    }

    fn play_playlist_item(&mut self, item: &PlaylistItem) -> Result<()> {
        let audio_stream = {
            let client = self.client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
        };

        if self.player.is_none() {
            self.player = Some(AudioPlayer::new()?);
        }

        if let Some(p) = &mut self.player {
            if let Some(mpris) = &self.mpris {
                mpris.set_track(item);
            }

            p.play(&audio_stream.url, self.settings.playback_speed)?;

            if let Some(mpris) = &self.mpris {
                mpris.set_state(PlayerState::Playing);
            }

            if let Screen::Library(library) = &mut self.screen {
                library.now_playing = Some((item.title.clone(), item.artist.clone()));
            }
        }

        self.apply_volume();
        Ok(())
    }

    fn toggle_playback(&mut self) -> Result<()> {
        if let Some(player) = &self.player {
            match player.state() {
                PlayerState::Playing => {
                    player.pause();
                    if let Some(mpris) = &self.mpris {
                        mpris.set_state(PlayerState::Paused);
                    }
                }
                PlayerState::Paused => {
                    player.resume();
                    if let Some(mpris) = &self.mpris {
                        mpris.set_state(PlayerState::Playing);
                    }
                }
                PlayerState::Stopped => {
                    self.start_playback_if_available()?;
                }
            }
        } else {
            self.start_playback_if_available()?;
        }
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        if let Some(player) = &mut self.player {
            player.stop();
            if let Some(mpris) = &self.mpris {
                mpris.set_state(PlayerState::Stopped);
            }
        }
        Ok(())
    }

    fn handle_mpris_command(&mut self, cmd: MprisCommand) -> Result<()> {
        match cmd {
            MprisCommand::Play => {
                if let Some(player) = &self.player {
                    match player.state() {
                        PlayerState::Paused => {
                            player.resume();
                            if let Some(mpris) = &self.mpris {
                                mpris.set_state(PlayerState::Playing);
                            }
                        }
                        PlayerState::Stopped => {
                            self.start_playback_if_available()?;
                        }
                        _ => {}
                    }
                } else {
                    self.start_playback_if_available()?;
                }
            }
            MprisCommand::Pause => {
                if let Some(player) = &self.player {
                    if player.state() == PlayerState::Playing {
                        player.pause();
                        if let Some(mpris) = &self.mpris {
                            mpris.set_state(PlayerState::Paused);
                        }
                    }
                }
            }
            MprisCommand::PlayPause => {
                self.toggle_playback()?;
            }
            MprisCommand::Stop => {
                self.stop()?;
            }
            MprisCommand::Next => {
                let next_item = self.playing_list.lock().advance_to_next().cloned();
                if let Some(item) = next_item {
                    self.play_playlist_item(&item)?;
                }
            }
            MprisCommand::Previous => {
                let prev_item = self.playing_list.lock().advance_to_previous().cloned();
                if let Some(item) = prev_item {
                    self.play_playlist_item(&item)?;
                }
            }
            MprisCommand::Seek(position) => {
                if let Some(player) = &self.player {
                    player.seek_to(position);
                    if let Some(mpris) = &self.mpris {
                        mpris.set_position(position);
                    }
                }
            }
            MprisCommand::SetPosition(position) => {
                if let Some(player) = &self.player {
                    player.seek_to(position);
                    if let Some(mpris) = &self.mpris {
                        mpris.set_position(position);
                    }
                }
            }
            MprisCommand::SetVolume(volume) => {
                self.settings.volume = (volume * 100.0) as u32;
                self.apply_volume();
                if let Some(mpris) = &self.mpris {
                    mpris.set_volume(volume as f32);
                }
            }
        }
        Ok(())
    }

    fn start_playback_if_available(&mut self) -> Result<()> {
        let item = {
            let mut list = self.playing_list.lock();
            if list.items().is_empty() {
                return Ok(());
            }
            if list.current().is_none() {
                list.jump_to(0);
            }
            list.current().cloned()
        };

        if let Some(item) = item {
            self.play_playlist_item(&item)?;
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
            library.load_data(self.client.clone())?;
        }
        Ok(())
    }

    fn load_library_data_into(&mut self, library: &mut LibraryScreen) -> Result<()> {
        library.load_data(self.client.clone())?;
        Ok(())
    }

    fn apply_volume(&mut self) {
        if let Some(player) = &self.player {
            player.set_volume(self.settings.volume_float());
        }
    }

    fn handle_seek_forward(&mut self) -> Result<()> {
        let should_stop = if let Some(player) = &self.player {
            let current_pos = player.position();
            let duration = player.duration();
            let new_pos = current_pos + Duration::from_secs(5);
            new_pos >= duration
        } else {
            return Ok(());
        };

        if should_stop {
            let item_to_play = {
                let mut list = self.playing_list.lock();
                match self.settings.loop_mode {
                    LoopMode::LoopOne => list.current().cloned(),
                    LoopMode::LoopList => {
                        let next_item = list.advance_to_next().cloned();
                        if next_item.is_some() {
                            next_item
                        } else {
                            let items = list.items();
                            if !items.is_empty() {
                                list.jump_to(0);
                                list.current().cloned()
                            } else {
                                None
                            }
                        }
                    }
                    LoopMode::NoLoop => None,
                }
            };

            if let Some(item) = item_to_play {
                self.play_playlist_item(&item)?;
            } else {
                if let Some(player) = &mut self.player {
                    player.stop();
                }
                if let Some(mpris) = &self.mpris {
                    mpris.set_state(PlayerState::Stopped);
                }
            }
        } else if let Some(player) = &self.player {
            player.seek_forward(Duration::from_secs(5));
        }
        Ok(())
    }

    fn handle_seek_backward(&mut self) -> Result<()> {
        let should_prev = if let Some(player) = &self.player {
            player.position() < Duration::from_secs(2)
        } else {
            return Ok(());
        };

        if should_prev {
            let item = {
                let mut list = self.playing_list.lock();
                let items_count = list.items().len();
                if items_count == 0 {
                    return Ok(());
                }

                let current_idx = list.current_index().unwrap_or(0);
                let prev_idx = if current_idx == 0 {
                    items_count - 1
                } else {
                    current_idx - 1
                };

                list.jump_to(prev_idx);
                list.current().cloned()
            };

            if let Some(item) = item {
                self.play_playlist_item(&item)?;
            }
        } else if let Some(player) = &self.player {
            player.seek_backward(Duration::from_secs(5));
        }
        Ok(())
    }

    /// Notify MPRIS when playback state changes from library operations.
    /// Compares old and new `now_playing` to detect if a new track started
    /// or playback stopped.
    fn notify_mpris_if_playback_changed(
        &self,
        old: &Option<(String, String)>,
        new: &Option<(String, String)>,
    ) {
        if new == old {
            return;
        }
        if let Some(mpris) = &self.mpris {
            match new {
                Some((title, artist)) => {
                    let duration = self
                        .player
                        .as_ref()
                        .map(|p| p.duration().as_secs() as u32)
                        .unwrap_or(0);
                    mpris.set_track_info(title, artist, duration);
                    mpris.set_state(PlayerState::Playing);
                }
                None => {
                    mpris.set_state(PlayerState::Stopped);
                }
            }
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
