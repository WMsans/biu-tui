use crate::api::{BilibiliClient, HistoryItem, WatchLaterItem};
use crate::api::{FavoriteFolder, FavoriteResource};
use crate::audio::AudioPlayer;
use crate::playing_list::{PlayingListManager, PlaylistItem};
use crate::storage::LoopMode;
use parking_lot::Mutex;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs},
    Frame,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LibraryTab {
    Favorites,
    WatchLater,
    History,
    PlayingList,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationLevel {
    Folders,
    Videos {
        folder_id: u64,
        folder_title: String,
    },
    Episodes {
        folder_id: u64,
        folder_id_title: String,
        bvid: String,
        video_title: String,
    },
}

pub enum NextAction {
    ReplayCurrent,
    PlayNext(usize),
}

#[derive(Clone)]
pub struct LibraryScreen {
    pub current_tab: LibraryTab,
    pub folders: Vec<FavoriteFolder>,
    pub resources: Vec<FavoriteResource>,
    pub episodes: Vec<crate::api::VideoPage>,
    pub watch_later: Vec<WatchLaterItem>,
    pub history: Vec<HistoryItem>,
    pub list_state: ListState,
    pub nav_level: NavigationLevel,
    pub now_playing: Option<(String, String)>,
    pub current_video_info: Option<crate::api::VideoInfo>,
    pub loop_mode: LoopMode,
    pub status_message: Option<String>,
    pub current_folder_page: u32,
    pub has_more_resources: bool,
    pub history_page: u32,
    pub has_more_history: bool,
    pub is_loading_more: bool,
}

impl Default for LibraryScreen {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryScreen {
    pub fn new() -> Self {
        Self {
            current_tab: LibraryTab::Favorites,
            folders: Vec::new(),
            resources: Vec::new(),
            episodes: Vec::new(),
            watch_later: Vec::new(),
            history: Vec::new(),
            list_state: ListState::default(),
            nav_level: NavigationLevel::Folders,
            now_playing: None,
            current_video_info: None,
            loop_mode: LoopMode::default(),
            status_message: None,
            current_folder_page: 1,
            has_more_resources: true,
            history_page: 1,
            has_more_history: true,
            is_loading_more: false,
        }
    }

    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
    }

    /// Resets the list_state selection to the first item of the current tab.
    /// Call this after switching tabs to avoid stale indices.
    pub fn reset_selection_for_tab(&mut self, playing_list: Arc<Mutex<PlayingListManager>>) {
        let len = self.current_list_len_with_playlist(&playing_list);
        if len > 0 {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    pub fn get_next_action(&self) -> Option<NextAction> {
        // TODO: This method is ready for use when song-end detection is implemented.
        // Currently the audio player runs independently without notifying the app when
        // playback completes. To wire up loop mode:
        // 1. Detect when current track ends (poll player.is_finished() or add callback)
        // 2. Call this method to determine next action based on loop_mode
        // 3. Execute the returned NextAction (replay current or play next)
        if self.resources.is_empty() {
            return None;
        }

        let current_idx = self.list_state.selected()?;

        match self.loop_mode {
            LoopMode::LoopOne => Some(NextAction::ReplayCurrent),
            LoopMode::NoLoop => {
                if current_idx + 1 < self.resources.len() {
                    Some(NextAction::PlayNext(current_idx + 1))
                } else {
                    None
                }
            }
            LoopMode::LoopList => {
                let next_idx = if current_idx + 1 < self.resources.len() {
                    current_idx + 1
                } else {
                    0
                };
                Some(NextAction::PlayNext(next_idx))
            }
        }
    }

    /// Loads favorites, watch later, and history data from the Bilibili API.
    ///
    /// Each API call locks the client, runs the request to completion, and
    /// releases the lock before the next call. This avoids the deadlock that
    /// occurred when `try_join!` interleaved futures that each held a
    /// `parking_lot::Mutex` guard across `.await` points.
    pub fn load_data(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;
        let mid = { client.lock().mid };

        let folders = if let Some(mid) = mid {
            let c = client.lock();
            rt.block_on(c.get_created_folders(mid))
                .map_err(|e| anyhow::anyhow!("Favorites API failed: {}", e))?
        } else {
            Vec::new()
        };

        let watch_later = {
            let c = client.lock();
            rt.block_on(c.get_watch_later())
                .map_err(|e| anyhow::anyhow!("Watch Later API failed: {}", e))?
        };

        let history = {
            let c = client.lock();
            rt.block_on(c.get_history(1))
                .map_err(|e| anyhow::anyhow!("History API failed: {}", e))?
        };

        self.folders = folders;
        self.watch_later = watch_later;
        self.history = history;

        // Initialize selection to the first item if data was loaded
        if !self.folders.is_empty() {
            self.list_state.select(Some(0));
        }

        Ok(())
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        player: Option<&AudioPlayer>,
        playing_list: Arc<Mutex<PlayingListManager>>,
    ) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(2), // now playing: 1 for border + 1 for content
                Constraint::Length(2), // progress bar: 1 for border + 1 for content
                Constraint::Length(2), // help bar: 1 for border + 1 for content
            ])
            .split(area);

        let titles: Vec<&str> = vec!["Favorites", "Watch Later", "History", "Playing List"];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.current_tab as usize)
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Cyan));
        f.render_widget(tabs, chunks[0]);

        let breadcrumb_text = match &self.nav_level {
            NavigationLevel::Folders => "Favorites".to_string(),
            NavigationLevel::Videos { folder_title, .. } => {
                format!("Favorites > {}", folder_title)
            }
            NavigationLevel::Episodes {
                folder_id_title,
                video_title,
                ..
            } => {
                format!("Favorites > {} > {}", folder_id_title, video_title)
            }
        };
        let breadcrumb = Paragraph::new(breadcrumb_text).style(Style::default().fg(Color::Yellow));
        f.render_widget(breadcrumb, chunks[1]);

        let items: Vec<ListItem> = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Folders => self
                    .folders
                    .iter()
                    .map(|f| ListItem::new(format!("{} ({})", f.title, f.media_count)))
                    .collect(),
                NavigationLevel::Videos { .. } => self
                    .resources
                    .iter()
                    .map(|r| {
                        let quality_badge = if r.duration > 300 { "[HQ]" } else { "" };
                        ListItem::new(format!(
                            "{} {}  {}  {:->5}  {}",
                            r.bvid,
                            r.title,
                            r.upper.name,
                            format_duration(r.duration),
                            quality_badge
                        ))
                    })
                    .collect(),
                NavigationLevel::Episodes { .. } => self
                    .episodes
                    .iter()
                    .map(|ep| {
                        ListItem::new(format!(
                            "P{} {}  {}",
                            ep.page,
                            ep.part,
                            format_duration(ep.duration),
                        ))
                    })
                    .collect(),
            },
            LibraryTab::WatchLater => self
                .watch_later
                .iter()
                .map(|w| {
                    ListItem::new(format!(
                        "{} - {}",
                        w.title,
                        w.owner
                            .as_ref()
                            .map(|o| o.name.as_str())
                            .unwrap_or("Unknown")
                    ))
                })
                .collect(),
            LibraryTab::History => self
                .history
                .iter()
                .map(|h| {
                    ListItem::new(format!(
                        "{} - {}",
                        h.title,
                        h.owner
                            .as_ref()
                            .map(|o| o.name.as_str())
                            .unwrap_or("Unknown")
                    ))
                })
                .collect(),
            LibraryTab::PlayingList => {
                let list = playing_list.lock();
                list.items()
                    .iter()
                    .enumerate()
                    .map(|(idx, item)| {
                        let current_marker = if Some(idx) == list.current_index() {
                            "♫"
                        } else {
                            " "
                        };
                        ListItem::new(format!(
                            "{} {} - {}  {}",
                            current_marker,
                            item.title,
                            item.artist,
                            format_duration(item.duration)
                        ))
                    })
                    .collect()
            }
        };

        let list = List::new(items)
            .block(Block::default().borders(Borders::ALL))
            .highlight_style(Style::default().bg(Color::DarkGray));
        f.render_stateful_widget(list, chunks[2], &mut self.list_state);

        let now_playing_text = if let Some((title, artist)) = &self.now_playing {
            format!("♫ Now Playing: {} - {}", title, artist)
        } else {
            "♫ Not Playing".to_string()
        };
        let now_playing = Paragraph::new(now_playing_text)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(now_playing, chunks[3]);

        use crate::audio::PlayerState;

        let (progress_text, progress_color) = if let Some(p) = player {
            let pos = p.position();
            let dur = p.duration();
            let pos_str = format_time(pos);
            let dur_str = format_time(dur);

            let progress = if dur.as_secs() > 0 {
                pos.as_secs_f32() / dur.as_secs_f32()
            } else {
                0.0
            };

            let width = chunks[4].width as usize;
            let bar_width = width.saturating_sub(20);
            let filled = (bar_width as f32 * progress) as usize;
            let filled = filled.min(bar_width);

            let bar: String = if bar_width > 0 {
                let filled_chars: String = std::iter::repeat_n('━', filled).collect();
                let empty_chars: String = std::iter::repeat_n('─', bar_width - filled).collect();
                format!("{}╾{}", filled_chars, empty_chars)
            } else {
                String::new()
            };

            let color = match p.state() {
                PlayerState::Paused => Color::Yellow,
                _ => Color::Cyan,
            };

            (format!("{}  {} / {}", bar, pos_str, dur_str), color)
        } else {
            ("━━──────────────  --:-- / --:--".to_string(), Color::Cyan)
        };

        let progress_bar = Paragraph::new(progress_text)
            .style(Style::default().fg(progress_color))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(progress_bar, chunks[4]);

        let (help_text, help_style) = if let Some(msg) = &self.status_message {
            (msg.clone(), Style::default().fg(Color::Green))
        } else {
            let text = match self.current_tab {
                LibraryTab::PlayingList => {
                    "[j/k] Navigate  [Enter] Jump  [d] Remove  [Tab] Switch"
                }
                _ => {
                    "[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [a] Add to list  [A] Add all  [Tab] Switch"
                }
            };
            (text.to_string(), Style::default())
        };
        let help = Paragraph::new(help_text)
            .style(help_style)
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[5]);
    }

    pub fn next_item(&mut self, playing_list: &Arc<Mutex<PlayingListManager>>) {
        let len = self.current_list_len_with_playlist(playing_list);
        if len > 0 {
            let i = self
                .list_state
                .selected()
                .map_or(0, |i| if i >= len - 1 { 0 } else { i + 1 });
            self.list_state.select(Some(i));
        }
    }

    pub fn prev_item(&mut self, playing_list: &Arc<Mutex<PlayingListManager>>) {
        let len = self.current_list_len_with_playlist(playing_list);
        if len > 0 {
            let i = self
                .list_state
                .selected()
                .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
            self.list_state.select(Some(i));
        }
    }

    fn current_list_len_with_playlist(
        &self,
        playing_list: &Arc<Mutex<PlayingListManager>>,
    ) -> usize {
        match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Folders => self.folders.len(),
                NavigationLevel::Videos { .. } => self.resources.len(),
                NavigationLevel::Episodes { .. } => self.episodes.len(),
            },
            LibraryTab::WatchLater => self.watch_later.len(),
            LibraryTab::History => self.history.len(),
            LibraryTab::PlayingList => playing_list.lock().items().len(),
        }
    }

    pub fn handle_jump_to_song(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
    ) -> anyhow::Result<()> {
        if self.current_tab != LibraryTab::PlayingList {
            return Ok(());
        }

        let selected_idx = match self.list_state.selected() {
            Some(idx) => idx,
            None => return Ok(()),
        };

        let item = {
            let mut list = playing_list.lock();
            list.jump_to(selected_idx);
            match list.current().cloned() {
                Some(item) => item,
                None => return Ok(()),
            }
        };

        let audio_stream = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
        };

        if player.is_none() {
            *player = Some(AudioPlayer::new()?);
        }

        if let Some(p) = player {
            p.play(&audio_stream.url)?;
            self.now_playing = Some((item.title, item.artist));
        }

        Ok(())
    }

    pub fn handle_enter(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playing_list: Arc<Mutex<PlayingListManager>>,
    ) -> anyhow::Result<()> {
        match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Folders => {
                    self.select_folder(client)?;
                }
                NavigationLevel::Videos {
                    folder_id,
                    folder_title,
                } => {
                    let folder_id = *folder_id;
                    let folder_title = folder_title.clone();
                    self.select_video_or_episodes(client, player, folder_id, folder_title)?;
                }
                NavigationLevel::Episodes { bvid, .. } => {
                    let bvid = bvid.clone();
                    self.play_episode(client, player, &bvid)?;
                }
            },
            LibraryTab::WatchLater | LibraryTab::History => {
                self.play_selected(client, player)?;
            }
            LibraryTab::PlayingList => {
                self.handle_jump_to_song(playing_list, client, player)?;
            }
        }
        Ok(())
    }

    fn select_folder(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.folders.len() {
                let folder = &self.folders[idx];
                let folder_id = folder.id;
                let folder_title = folder.title.clone();

                let resources = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_folder_resources(folder_id, 1))?
                };

                self.resources = resources.0;
                self.nav_level = NavigationLevel::Videos {
                    folder_id,
                    folder_title,
                };
                self.list_state.select(Some(0));
            }
        }
        Ok(())
    }

    fn select_video_or_episodes(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        folder_id: u64,
        folder_title: String,
    ) -> anyhow::Result<()> {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.resources.len() {
                let resource = &self.resources[idx];
                let bvid = resource.bvid.clone();

                let video_info = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_video_info(&bvid))?
                };

                if video_info.pages.len() > 1 {
                    let video_title = video_info.title.clone();
                    self.episodes = video_info.pages.clone();
                    self.current_video_info = Some(video_info);
                    self.nav_level = NavigationLevel::Episodes {
                        folder_id,
                        folder_id_title: folder_title,
                        bvid,
                        video_title,
                    };
                    self.list_state.select(Some(0));
                } else {
                    // Single-page video: play directly
                    self.play_video(client, player, &video_info)?;
                }
            }
        }
        Ok(())
    }

    fn play_episode(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        bvid: &str,
    ) -> anyhow::Result<()> {
        if let Some(idx) = self.list_state.selected() {
            if idx < self.episodes.len() {
                let episode = &self.episodes[idx];
                let cid = episode.cid;
                let episode_title = episode.part.clone();

                let audio_stream = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_best_audio(bvid, cid))?
                };

                if player.is_none() {
                    *player = Some(AudioPlayer::new()?);
                }

                if let Some(p) = player {
                    p.play(&audio_stream.url)?;
                    // Show the episode title with the video owner name
                    let owner_name = self
                        .current_video_info
                        .as_ref()
                        .map(|v| v.owner.name.clone())
                        .unwrap_or_default();
                    self.now_playing = Some((episode_title, owner_name));
                }
            }
        }
        Ok(())
    }

    fn play_video(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        video_info: &crate::api::VideoInfo,
    ) -> anyhow::Result<()> {
        let audio_stream = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            rt.block_on(client.get_best_audio(&video_info.bvid, video_info.cid))?
        };

        if player.is_none() {
            *player = Some(AudioPlayer::new()?);
        }

        if let Some(p) = player {
            p.play(&audio_stream.url)?;
            self.now_playing = Some((video_info.title.clone(), video_info.owner.name.clone()));
        }

        Ok(())
    }

    fn play_selected(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
    ) -> anyhow::Result<()> {
        let bvid = match self.current_tab {
            LibraryTab::Favorites => {
                if let Some(idx) = self.list_state.selected() {
                    self.resources.get(idx).map(|r| r.bvid.clone())
                } else {
                    None
                }
            }
            LibraryTab::WatchLater => {
                if let Some(idx) = self.list_state.selected() {
                    self.watch_later.get(idx).map(|w| w.bvid.clone())
                } else {
                    None
                }
            }
            LibraryTab::History => {
                if let Some(idx) = self.list_state.selected() {
                    self.history.get(idx).and_then(|h| h.bvid.clone())
                } else {
                    None
                }
            }
            LibraryTab::PlayingList => None,
        };

        if let Some(bvid) = bvid {
            let (video_info, audio_stream) = {
                let client = client.lock();
                let rt = tokio::runtime::Runtime::new()?;

                let video_info = rt.block_on(client.get_video_info(&bvid))?;
                let cid = video_info.cid;

                let audio_stream = rt.block_on(client.get_best_audio(&bvid, cid))?;

                (video_info, audio_stream)
            };

            if player.is_none() {
                *player = Some(AudioPlayer::new()?);
            }

            if let Some(p) = player {
                p.play(&audio_stream.url)?;
                self.now_playing = Some((video_info.title, video_info.owner.name));
            }
        }

        Ok(())
    }

    pub fn add_to_playing_list(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<()> {
        let idx = match self.list_state.selected() {
            Some(idx) => idx,
            None => {
                self.status_message = Some("No item selected".to_string());
                return Ok(());
            }
        };

        // For Episodes, we already have the cid from the page data
        if let LibraryTab::Favorites = self.current_tab {
            if let NavigationLevel::Episodes { bvid, .. } = &self.nav_level {
                if let Some(episode) = self.episodes.get(idx) {
                    let artist = self
                        .current_video_info
                        .as_ref()
                        .map(|v| v.owner.name.clone())
                        .unwrap_or_default();
                    let item = PlaylistItem {
                        bvid: bvid.clone(),
                        cid: episode.cid,
                        title: episode.part.clone(),
                        artist,
                        duration: episode.duration,
                    };
                    let title = item.title.clone();
                    playing_list.lock().add(item);
                    self.status_message = Some(format!("Added: {}", title));
                    return Ok(());
                }
                self.status_message = Some("No episode at this index".to_string());
                return Ok(());
            }
        }

        // For other cases, extract metadata and fetch CID from API
        let Some((bvid, title, artist, duration)) = (match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Videos { .. } => self.resources.get(idx).map(|r| {
                    (
                        r.bvid.clone(),
                        r.title.clone(),
                        r.upper.name.clone(),
                        r.duration,
                    )
                }),
                NavigationLevel::Folders => {
                    self.status_message =
                        Some("Navigate into a folder first to add songs".to_string());
                    return Ok(());
                }
                NavigationLevel::Episodes { .. } => unreachable!(), // handled above
            },
            LibraryTab::WatchLater => self.watch_later.get(idx).map(|w| {
                (
                    w.bvid.clone(),
                    w.title.clone(),
                    w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                    w.duration,
                )
            }),
            LibraryTab::History => self.history.get(idx).and_then(|h| {
                h.bvid.as_ref().map(|bvid| {
                    (
                        bvid.clone(),
                        h.title.clone(),
                        h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                        h.duration,
                    )
                })
            }),
            LibraryTab::PlayingList => {
                self.status_message = Some("Already in playing list".to_string());
                return Ok(());
            }
        }) else {
            self.status_message = Some("No valid item at this index".to_string());
            return Ok(());
        };

        let cid = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            let video_info = rt.block_on(client.get_video_info(&bvid))?;
            video_info.cid
        };

        let item = PlaylistItem {
            bvid,
            cid,
            title: title.clone(),
            artist,
            duration,
        };

        playing_list.lock().add(item);
        self.status_message = Some(format!("Added: {}", title));

        Ok(())
    }

    pub fn add_all_to_playing_list(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<()> {
        let rt = tokio::runtime::Runtime::new()?;

        let items: Vec<PlaylistItem> = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Videos { .. } => {
                    let mut items = Vec::new();
                    for resource in &self.resources {
                        let cid = {
                            let client = client.lock();
                            let video_info = rt.block_on(client.get_video_info(&resource.bvid))?;
                            video_info.cid
                        };

                        items.push(PlaylistItem {
                            bvid: resource.bvid.clone(),
                            cid,
                            title: resource.title.clone(),
                            artist: resource.upper.name.clone(),
                            duration: resource.duration,
                        });
                    }
                    items
                }
                NavigationLevel::Episodes { bvid, .. } => {
                    let artist = self
                        .current_video_info
                        .as_ref()
                        .map(|v| v.owner.name.clone())
                        .unwrap_or_default();
                    self.episodes
                        .iter()
                        .map(|ep| PlaylistItem {
                            bvid: bvid.clone(),
                            cid: ep.cid,
                            title: ep.part.clone(),
                            artist: artist.clone(),
                            duration: ep.duration,
                        })
                        .collect()
                }
                NavigationLevel::Folders => {
                    self.status_message =
                        Some("Navigate into a folder first to add songs".to_string());
                    return Ok(());
                }
            },
            LibraryTab::WatchLater => {
                let mut items = Vec::new();
                for w in &self.watch_later {
                    let cid = {
                        let client = client.lock();
                        let video_info = rt.block_on(client.get_video_info(&w.bvid))?;
                        video_info.cid
                    };
                    items.push(PlaylistItem {
                        bvid: w.bvid.clone(),
                        cid,
                        title: w.title.clone(),
                        artist: w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                        duration: w.duration,
                    });
                }
                items
            }
            LibraryTab::History => {
                let mut items = Vec::new();
                for h in &self.history {
                    if let Some(bvid) = &h.bvid {
                        let cid = {
                            let client = client.lock();
                            let video_info = rt.block_on(client.get_video_info(bvid))?;
                            video_info.cid
                        };
                        items.push(PlaylistItem {
                            bvid: bvid.clone(),
                            cid,
                            title: h.title.clone(),
                            artist: h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                            duration: h.duration,
                        });
                    }
                }
                items
            }
            LibraryTab::PlayingList => {
                self.status_message = Some("Already in playing list".to_string());
                return Ok(());
            }
        };

        let count = items.len();
        if !items.is_empty() {
            playing_list.lock().add_all(items);
            self.status_message = Some(format!("Added {} songs to playing list", count));
        } else {
            self.status_message = Some("No songs to add".to_string());
        }

        Ok(())
    }

    pub fn go_back(&mut self) {
        match &self.nav_level {
            NavigationLevel::Videos { .. } => {
                self.nav_level = NavigationLevel::Folders;
                self.resources.clear();
                self.list_state.select(Some(0));
            }
            NavigationLevel::Episodes {
                folder_id,
                folder_id_title,
                ..
            } => {
                self.nav_level = NavigationLevel::Videos {
                    folder_id: *folder_id,
                    folder_title: folder_id_title.clone(),
                };
                self.episodes.clear();
                self.list_state.select(Some(0));
            }
            NavigationLevel::Folders => {}
        }
    }

    pub fn handle_remove_song(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
    ) -> anyhow::Result<()> {
        if self.current_tab != LibraryTab::PlayingList {
            return Ok(());
        }

        let selected_idx = match self.list_state.selected() {
            Some(idx) => idx,
            None => return Ok(()),
        };

        let current_idx = playing_list.lock().current_index();
        let is_current = current_idx == Some(selected_idx);

        playing_list.lock().remove(selected_idx);

        if is_current {
            let next_item = playing_list.lock().current().cloned();

            if let Some(item) = next_item {
                let audio_stream = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
                };

                if let Some(p) = player {
                    p.play(&audio_stream.url)?;
                    self.now_playing = Some((item.title, item.artist));
                }
            } else {
                if let Some(p) = player {
                    p.stop();
                }
                self.now_playing = None;
            }
        }

        let list_len = playing_list.lock().items().len();
        if selected_idx >= list_len && list_len > 0 {
            self.list_state.select(Some(list_len - 1));
        }

        Ok(())
    }
}

fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", mins, secs)
}

fn format_time(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{:02}:{:02}", mins, secs)
}
