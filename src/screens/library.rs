use crate::api::{BilibiliClient, HistoryItem, WatchLaterItem};
use crate::api::{FavoriteFolder, FavoriteResource};
use crate::audio::AudioPlayer;
use crate::playing_list::{PlayingListManager, PlaylistItem};
use crate::playlists::PlaylistManager;
use crate::storage::LoopMode;
use crate::ui::widgets::SearchBar;
use crossterm::event::KeyCode;
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
    PlayingNow,
    Playlists,
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

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistNavLevel {
    PlaylistList,
    PlaylistContents { name: String },
}

#[derive(Debug, Clone)]
pub struct AddPromptState {
    pub is_active: bool,
    pub destinations: Vec<String>,
    pub selected: usize,
    pub source_title: String,
    pub mode: AddPromptMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AddPromptMode {
    Single,
    All,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PlaylistEditMode {
    Creating,
    Renaming { old_name: String },
}

pub enum NextAction {
    ReplayCurrent,
    PlayNext(usize),
}

const SCROLL_PADDING: usize = 3;

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
    visible_height: usize,
    pub search_state: Option<crate::screens::SearchState>,
    pub original_folders: Option<Vec<FavoriteFolder>>,
    pub original_resources: Option<Vec<FavoriteResource>>,
    pub original_episodes: Option<Vec<crate::api::VideoPage>>,
    pub original_watch_later: Option<Vec<WatchLaterItem>>,
    pub original_history: Option<Vec<HistoryItem>>,
    pub playlist_nav_level: PlaylistNavLevel,
    pub playlist_items: Vec<PlaylistItem>,
    pub playlist_names: Vec<String>,
    pub add_prompt_state: Option<AddPromptState>,
    pub playlist_edit_mode: Option<PlaylistEditMode>,
    pub add_all_candidates: Vec<PlaylistItem>,
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
            visible_height: 0,
            search_state: None,
            original_folders: None,
            original_resources: None,
            original_episodes: None,
            original_watch_later: None,
            original_history: None,
            playlist_nav_level: PlaylistNavLevel::PlaylistList,
            playlist_items: Vec::new(),
            playlist_names: Vec::new(),
            add_prompt_state: None,
            playlist_edit_mode: None,
            add_all_candidates: Vec::new(),
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

        // Initialize history pagination
        self.history_page = 1;
        self.has_more_history = self.history.len() >= 20;

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
        playlist_manager: Arc<Mutex<PlaylistManager>>,
    ) {
        // Refresh playlist names if on Playlists tab
        if self.current_tab == LibraryTab::Playlists {
            if let Ok(names) = playlist_manager.lock().list_names() {
                self.playlist_names = names;
            }
        }

        let show_breadcrumb = matches!(
            self.current_tab,
            LibraryTab::Favorites | LibraryTab::Playlists
        );

        let mut constraints = vec![Constraint::Length(3)];
        if show_breadcrumb {
            constraints.push(Constraint::Length(1));
        }
        if self.search_state.is_some() {
            constraints.push(Constraint::Length(3));
        }
        constraints.extend(vec![
            Constraint::Min(10),
            Constraint::Length(2),
            Constraint::Length(2),
            Constraint::Length(2),
        ]);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let titles: Vec<&str> = vec![
            "Favorites",
            "Watch Later",
            "History",
            "Playing Now",
            "Playlists",
        ];
        let tabs = Tabs::new(titles)
            .block(Block::default().borders(Borders::BOTTOM))
            .select(self.current_tab as usize)
            .style(Style::default())
            .highlight_style(Style::default().fg(Color::Cyan));
        f.render_widget(tabs, chunks[0]);

        let mut chunk_offset = 1;

        if show_breadcrumb {
            let breadcrumb_text = match self.current_tab {
                LibraryTab::Favorites => match &self.nav_level {
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
                },
                LibraryTab::Playlists => match &self.playlist_nav_level {
                    PlaylistNavLevel::PlaylistList => "Playlists".to_string(),
                    PlaylistNavLevel::PlaylistContents { name } => {
                        format!("Playlists > {}", name)
                    }
                },
                _ => String::new(),
            };
            let breadcrumb =
                Paragraph::new(breadcrumb_text).style(Style::default().fg(Color::Yellow));
            f.render_widget(breadcrumb, chunks[chunk_offset]);
            chunk_offset += 1;
        }

        if let Some(ref search_state) = self.search_state {
            SearchBar::new(&search_state.query, search_state.cursor_position)
                .render(f, chunks[chunk_offset]);
            chunk_offset += 1;
        }

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
            LibraryTab::PlayingNow => {
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
            LibraryTab::Playlists => match &self.playlist_nav_level {
                PlaylistNavLevel::PlaylistList => {
                    if self.playlist_names.is_empty() {
                        vec![ListItem::new("No playlists yet. Press 'n' to create one.")]
                    } else {
                        let pm = playlist_manager.lock();
                        self.playlist_names
                            .iter()
                            .map(|name| {
                                let count =
                                    pm.get_items(name).map(|items| items.len()).unwrap_or(0);
                                ListItem::new(format!("{} ({})", name, count))
                            })
                            .collect()
                    }
                }
                PlaylistNavLevel::PlaylistContents { .. } => self
                    .playlist_items
                    .iter()
                    .map(|item| {
                        ListItem::new(format!(
                            "{} - {}  {}",
                            item.title,
                            item.artist,
                            format_duration(item.duration)
                        ))
                    })
                    .collect(),
            },
        };

        let list = if items.is_empty() {
            if let Some(ref search_state) = self.search_state {
                let query = search_state.query.as_str();
                List::new(vec![ListItem::new(format!(
                    "No results found for '{}'",
                    query
                ))])
                .block(Block::default().borders(Borders::ALL))
            } else {
                List::new(items)
                    .block(Block::default().borders(Borders::ALL))
                    .highlight_style(Style::default().bg(Color::DarkGray))
            }
        } else {
            List::new(items)
                .block(Block::default().borders(Borders::ALL))
                .highlight_style(Style::default().bg(Color::DarkGray))
        };

        let visible_height = chunks[chunk_offset].height.saturating_sub(2) as usize;
        self.adjust_scroll_offset(visible_height);
        f.render_stateful_widget(list, chunks[chunk_offset], &mut self.list_state);
        self.visible_height = visible_height;
        chunk_offset += 1;

        let now_playing_text = if let Some((title, artist)) = &self.now_playing {
            format!("♫ Now Playing: {} - {}", title, artist)
        } else {
            "♫ Not Playing".to_string()
        };
        let now_playing = Paragraph::new(now_playing_text)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(now_playing, chunks[chunk_offset]);

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

            let width = chunks[chunk_offset + 1].width as usize;
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
        f.render_widget(progress_bar, chunks[chunk_offset + 1]);

        let (help_text, help_style) = if let Some(msg) = &self.status_message {
            (msg.clone(), Style::default().fg(Color::Green))
        } else if self.search_state.is_some() {
            (
                "[Enter] Search  [Esc] Cancel  [Ctrl+U] Clear".to_string(),
                Style::default(),
            )
        } else {
            let text = match self.current_tab {
                LibraryTab::PlayingNow => {
                    "[j/k] Navigate  [Enter] Jump  [d] Remove  [Tab] Switch  [/] Search"
                }
                LibraryTab::Playlists => match &self.playlist_nav_level {
                    PlaylistNavLevel::PlaylistList => {
                        "[j/k] Navigate  [Enter] Open  [n] New  [d] Delete  [r] Rename  [Tab] Switch"
                    }
                    PlaylistNavLevel::PlaylistContents { .. } => {
                        "[j/k] Navigate  [Enter] Play  [d] Remove  [Ctrl+↑/↓] Reorder  [Esc] Back  [Tab] Switch"
                    }
                },
                _ => {
                    "[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [a] Add to list  [A] Add all  [Tab] Switch  [/] Search"
                }
            };
            (text.to_string(), Style::default())
        };
        let help = Paragraph::new(help_text)
            .style(help_style)
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[chunk_offset + 2]);

        if let Some(ref prompt) = self.add_prompt_state {
            if prompt.is_active {
                let prompt_title = match prompt.mode {
                    AddPromptMode::Single => format!("Add '{}' to:", prompt.source_title),
                    AddPromptMode::All => {
                        format!("Add {} items to:", self.add_all_candidates.len())
                    }
                };
                let mut prompt_lines: Vec<String> = vec![prompt_title, String::new()];
                for (i, dest) in prompt.destinations.iter().enumerate() {
                    let marker = if i == prompt.selected { "> " } else { "  " };
                    prompt_lines.push(format!("{}{}", marker, dest));
                }
                prompt_lines.push(String::new());
                prompt_lines.push("[j/k] Select  [Enter] Confirm  [Esc] Cancel".to_string());

                let prompt_height = prompt_lines.len() as u16 + 2;
                let prompt_width = 50u16;
                let prompt_x = area.width.saturating_sub(prompt_width) / 2;
                let prompt_y = area.height.saturating_sub(prompt_height) / 2;
                let prompt_area = Rect::new(prompt_x, prompt_y, prompt_width, prompt_height);

                let prompt_text = prompt_lines.join("\n");
                let prompt_widget = Paragraph::new(prompt_text)
                    .block(Block::default().borders(Borders::ALL).title("Add To"))
                    .style(Style::default().fg(Color::Yellow));
                f.render_widget(prompt_widget, prompt_area);
            }
        }
    }

    pub fn next_item(
        &mut self,
        playing_list: &Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) {
        let len = self.current_list_len_with_playlist(playing_list);
        if len > 0 {
            let current = self.list_state.selected().unwrap_or(0);
            let next_idx = if current >= len - 1 { 0 } else { current + 1 };
            self.list_state.select(Some(next_idx));

            if next_idx == len - 1 {
                self.try_load_more(client);
            }
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

    fn adjust_scroll_offset(&mut self, height: usize) {
        let selected = match self.list_state.selected() {
            Some(s) => s,
            None => return,
        };

        if height == 0 {
            return;
        }

        let current_offset = self.list_state.offset();
        let max_offset = selected.saturating_sub(SCROLL_PADDING);
        let min_offset = (selected + SCROLL_PADDING + 1).saturating_sub(height);

        let new_offset = if min_offset <= max_offset {
            current_offset.clamp(min_offset, max_offset)
        } else {
            max_offset
        };
        *self.list_state.offset_mut() = new_offset;
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
            LibraryTab::PlayingNow => playing_list.lock().items().len(),
            LibraryTab::Playlists => match &self.playlist_nav_level {
                PlaylistNavLevel::PlaylistList => self.playlist_names.len(),
                PlaylistNavLevel::PlaylistContents { .. } => self.playlist_items.len(),
            },
        }
    }

    pub fn handle_jump_to_song(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playback_speed: f32,
    ) -> anyhow::Result<()> {
        if self.current_tab != LibraryTab::PlayingNow {
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
            p.play(&audio_stream.url, playback_speed)?;
            self.now_playing = Some((item.title, item.artist));
        }

        Ok(())
    }

    pub fn handle_enter(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        playback_speed: f32,
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
                    self.select_video_or_episodes(
                        client,
                        player,
                        folder_id,
                        folder_title,
                        playback_speed,
                    )?;
                }
                NavigationLevel::Episodes { bvid, .. } => {
                    let bvid = bvid.clone();
                    self.play_episode(client, player, &bvid, playback_speed)?;
                }
            },
            LibraryTab::WatchLater | LibraryTab::History => {
                self.play_selected(client, player, playback_speed)?;
            }
            LibraryTab::PlayingNow => {
                self.handle_jump_to_song(playing_list, client, player, playback_speed)?;
            }
            LibraryTab::Playlists => {
                self.handle_playlists_enter(
                    playing_list,
                    playlist_manager,
                    client,
                    player,
                    playback_speed,
                )?;
            }
        }
        Ok(())
    }

    fn handle_playlists_enter(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playback_speed: f32,
    ) -> anyhow::Result<()> {
        match &self.playlist_nav_level.clone() {
            PlaylistNavLevel::PlaylistList => {
                let idx = match self.list_state.selected() {
                    Some(i) if i < self.playlist_names.len() => i,
                    _ => return Ok(()),
                };
                let name = self.playlist_names[idx].clone();
                let pm = playlist_manager.lock();
                self.playlist_items = pm.get_items(&name)?;
                drop(pm);
                self.playlist_nav_level = PlaylistNavLevel::PlaylistContents { name };
                self.list_state.select(Some(0));
            }
            PlaylistNavLevel::PlaylistContents { name } => {
                let name = name.clone();
                let idx = match self.list_state.selected() {
                    Some(i) if i < self.playlist_items.len() => i,
                    _ => {
                        self.status_message = Some("No tracks in playlist".to_string());
                        return Ok(());
                    }
                };
                let items = self.playlist_items.clone();
                {
                    let mut pl = playing_list.lock();
                    pl.clear();
                    pl.add_all(items);
                    pl.jump_to(idx);
                }
                let item = &self.playlist_items[idx];
                let audio_stream = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_best_audio(&item.bvid, item.cid))?
                };
                if player.is_none() {
                    *player = Some(AudioPlayer::new()?);
                }
                if let Some(p) = player {
                    p.play(&audio_stream.url, playback_speed)?;
                    self.now_playing = Some((item.title.clone(), item.artist.clone()));
                }
                self.status_message = Some(format!("Loaded '{}' into Playing Now", name));
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

                self.current_folder_page = 1;

                let resources = {
                    let client = client.lock();
                    let rt = tokio::runtime::Runtime::new()?;
                    rt.block_on(client.get_folder_resources(folder_id, 1))?
                };

                self.resources = resources.0;
                self.has_more_resources = resources.1;
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
        playback_speed: f32,
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
                    self.play_video(client, player, &video_info, playback_speed)?;
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
        playback_speed: f32,
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
                    p.play(&audio_stream.url, playback_speed)?;
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
        playback_speed: f32,
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
            p.play(&audio_stream.url, playback_speed)?;
            self.now_playing = Some((video_info.title.clone(), video_info.owner.name.clone()));
        }

        Ok(())
    }

    fn play_selected(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playback_speed: f32,
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
            LibraryTab::PlayingNow => None,
            LibraryTab::Playlists => None,
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
                p.play(&audio_stream.url, playback_speed)?;
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
            LibraryTab::PlayingNow => {
                self.status_message = Some("Already in playing list".to_string());
                return Ok(());
            }
            LibraryTab::Playlists => {
                self.status_message =
                    Some("Use 'a' from other tabs to add to playlists".to_string());
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
            LibraryTab::PlayingNow => {
                self.status_message = Some("Already in playing list".to_string());
                return Ok(());
            }
            LibraryTab::Playlists => {
                self.status_message =
                    Some("Use 'A' from other tabs to add all to playlists".to_string());
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
        if self.current_tab == LibraryTab::Playlists {
            match &self.playlist_nav_level {
                PlaylistNavLevel::PlaylistContents { .. } => {
                    self.playlist_nav_level = PlaylistNavLevel::PlaylistList;
                    self.playlist_items.clear();
                    self.list_state.select(Some(0));
                }
                PlaylistNavLevel::PlaylistList => {}
            }
            return;
        }

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

    fn try_load_more(&mut self, client: Arc<Mutex<BilibiliClient>>) {
        if self.is_loading_more {
            return;
        }

        match self.current_tab {
            LibraryTab::Favorites => {
                if let NavigationLevel::Videos { folder_id, .. } = &self.nav_level {
                    self.try_load_more_favorites(client, *folder_id);
                }
            }
            LibraryTab::History => self.try_load_more_history(client),
            _ => {}
        }
    }

    fn try_load_more_favorites(&mut self, client: Arc<Mutex<BilibiliClient>>, folder_id: u64) {
        if !self.has_more_resources || self.is_loading_more {
            return;
        }

        self.is_loading_more = true;
        self.status_message = Some("Loading more...".to_string());

        let next_page = self.current_folder_page + 1;

        let result = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new().ok();
            rt.and_then(|rt| {
                rt.block_on(client.get_folder_resources(folder_id, next_page))
                    .ok()
            })
        };

        match result {
            Some((new_resources, has_more)) => {
                self.resources.extend(new_resources);
                self.current_folder_page = next_page;
                self.has_more_resources = has_more;
                self.status_message = None;
            }
            None => {
                self.status_message = Some("Failed to load more".to_string());
            }
        }

        self.is_loading_more = false;
    }

    fn try_load_more_history(&mut self, client: Arc<Mutex<BilibiliClient>>) {
        if !self.has_more_history || self.is_loading_more {
            return;
        }

        self.is_loading_more = true;
        self.status_message = Some("Loading more...".to_string());

        let next_page = self.history_page + 1;

        let result = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new().ok();
            rt.and_then(|rt| rt.block_on(client.get_history(next_page)).ok())
        };

        match result {
            Some(new_history) => {
                self.has_more_history = new_history.len() >= 20;
                self.history.extend(new_history);
                self.history_page = next_page;
                self.status_message = None;
            }
            None => {
                self.status_message = Some("Failed to load more".to_string());
            }
        }

        self.is_loading_more = false;
    }

    pub fn handle_remove_song(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
        playback_speed: f32,
    ) -> anyhow::Result<()> {
        if self.current_tab != LibraryTab::PlayingNow && self.current_tab != LibraryTab::Playlists {
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
                    p.play(&audio_stream.url, playback_speed)?;
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

    pub fn handle_playlists_create(&mut self, _playlist_manager: Arc<Mutex<PlaylistManager>>) {
        if self.search_state.is_some() {
            return;
        }
        self.playlist_edit_mode = Some(PlaylistEditMode::Creating);
        self.search_state = Some(crate::screens::SearchState::new());
        self.status_message =
            Some("Type playlist name and press Enter to create, Esc to cancel".to_string());
    }

    pub fn handle_playlists_delete(&mut self, playlist_manager: Arc<Mutex<PlaylistManager>>) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_names.len() => i,
            _ => return,
        };
        let name = self.playlist_names[idx].clone();
        match playlist_manager.lock().delete(&name) {
            Ok(_) => {
                self.playlist_names.remove(idx);
                if idx >= self.playlist_names.len() && !self.playlist_names.is_empty() {
                    self.list_state.select(Some(self.playlist_names.len() - 1));
                } else if self.playlist_names.is_empty() {
                    self.list_state.select(None);
                }
                self.status_message = Some(format!("Deleted '{}'", name));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to delete: {}", e));
            }
        }
    }

    pub fn handle_playlists_rename(&mut self, _playlist_manager: Arc<Mutex<PlaylistManager>>) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_names.len() => i,
            _ => return,
        };
        let name = self.playlist_names[idx].clone();
        if self.search_state.is_some() {
            return;
        }
        self.playlist_edit_mode = Some(PlaylistEditMode::Renaming {
            old_name: name.clone(),
        });
        self.search_state = Some(crate::screens::SearchState::new());
        self.status_message = Some(format!(
            "Renaming '{}': type new name and press Enter",
            name
        ));
    }

    pub fn handle_playlists_remove_item(&mut self, playlist_manager: Arc<Mutex<PlaylistManager>>) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_items.len() => i,
            _ => return,
        };
        let name = match &self.playlist_nav_level {
            PlaylistNavLevel::PlaylistContents { name } => name.clone(),
            _ => return,
        };
        match playlist_manager.lock().remove_item(&name, idx) {
            Ok(_) => {
                self.playlist_items.remove(idx);
                if idx >= self.playlist_items.len() && !self.playlist_items.is_empty() {
                    self.list_state.select(Some(self.playlist_items.len() - 1));
                } else if self.playlist_items.is_empty() {
                    self.list_state.select(None);
                }
                self.status_message = Some("Removed from playlist".to_string());
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to remove: {}", e));
            }
        }
    }

    pub fn handle_playlists_reorder(
        &mut self,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        direction_up: bool,
    ) {
        let idx = match self.list_state.selected() {
            Some(i) if i < self.playlist_items.len() => i,
            _ => return,
        };
        let name = match &self.playlist_nav_level {
            PlaylistNavLevel::PlaylistContents { name } => name.clone(),
            _ => return,
        };

        let to = if direction_up {
            if idx == 0 {
                return;
            }
            idx - 1
        } else {
            if idx + 1 >= self.playlist_items.len() {
                return;
            }
            idx + 1
        };

        match playlist_manager.lock().reorder(&name, idx, to) {
            Ok(_) => {
                let item = self.playlist_items.remove(idx);
                self.playlist_items.insert(to, item);
                self.list_state.select(Some(to));
            }
            Err(e) => {
                self.status_message = Some(format!("Failed to reorder: {}", e));
            }
        }
    }

    pub fn toggle_add_prompt(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
        mode: AddPromptMode,
    ) {
        if self.add_prompt_state.is_some() {
            self.add_prompt_state = None;
            return;
        }

        let idx = match self.list_state.selected() {
            Some(i) => i,
            None => {
                self.status_message = Some("No item selected".to_string());
                return;
            }
        };

        if mode == AddPromptMode::All {
            let candidates: Vec<PlaylistItem> = match self.current_tab {
                LibraryTab::Favorites => match &self.nav_level {
                    NavigationLevel::Videos { .. } => self
                        .resources
                        .iter()
                        .map(|r| PlaylistItem {
                            bvid: r.bvid.clone(),
                            cid: 0,
                            title: r.title.clone(),
                            artist: r.upper.name.clone(),
                            duration: r.duration,
                        })
                        .collect(),
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
                    _ => Vec::new(),
                },
                LibraryTab::WatchLater => self
                    .watch_later
                    .iter()
                    .map(|w| PlaylistItem {
                        bvid: w.bvid.clone(),
                        cid: 0,
                        title: w.title.clone(),
                        artist: w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                        duration: w.duration,
                    })
                    .collect(),
                LibraryTab::History => self
                    .history
                    .iter()
                    .filter_map(|h| {
                        h.bvid.as_ref().map(|bvid| PlaylistItem {
                            bvid: bvid.clone(),
                            cid: 0,
                            title: h.title.clone(),
                            artist: h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                            duration: h.duration,
                        })
                    })
                    .collect(),
                _ => Vec::new(),
            };
            if candidates.is_empty() {
                self.status_message = Some("No items to add".to_string());
                return;
            }
            self.add_all_candidates = candidates;
        }

        let source_title = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Folders => {
                    self.status_message = Some("Navigate into a folder first".to_string());
                    return;
                }
                NavigationLevel::Videos { .. } => self
                    .resources
                    .get(idx)
                    .map(|r| r.title.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
                NavigationLevel::Episodes { .. } => self
                    .episodes
                    .get(idx)
                    .map(|e| e.part.clone())
                    .unwrap_or_else(|| "Unknown".to_string()),
            },
            LibraryTab::WatchLater => self
                .watch_later
                .get(idx)
                .map(|w| w.title.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            LibraryTab::History => self
                .history
                .get(idx)
                .map(|h| h.title.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            _ => {
                self.status_message = Some("Nothing to add here".to_string());
                return;
            }
        };

        let mut destinations = vec!["Playing Now".to_string()];
        if let Ok(names) = playlist_manager.lock().list_names() {
            destinations.extend(names);
        }

        if destinations.len() <= 1 {
            if mode == AddPromptMode::All {
                let _ = self.add_all_to_playing_list(playing_list, client);
            } else {
                let _ = self.add_to_playing_list(playing_list, client);
            }
            return;
        }

        self.add_prompt_state = Some(AddPromptState {
            is_active: true,
            destinations,
            selected: 0,
            source_title,
            mode,
        });
    }

    pub fn resolve_add_target(
        &mut self,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<()> {
        let prompt = match &self.add_prompt_state {
            Some(p) if p.is_active => p.clone(),
            _ => return Ok(()),
        };

        let selected_dest = prompt.destinations[prompt.selected].clone();

        if selected_dest == "Playing Now" {
            if prompt.mode == AddPromptMode::All {
                return self.add_all_to_playing_list(playing_list, client);
            }
            self.add_prompt_state = None;
            let result = self.add_to_playing_list(playing_list, client);
            self.status_message = Some("Added to Playing Now".to_string());
            return result;
        }

        if prompt.mode == AddPromptMode::All {
            let count = self.add_all_candidates.len();
            let rt = tokio::runtime::Runtime::new()?;
            for mut item in self.add_all_candidates.clone() {
                if item.cid == 0 {
                    let cid = {
                        let client = client.lock();
                        rt.block_on(client.get_video_info(&item.bvid))?.cid
                    };
                    item.cid = cid;
                }
                playlist_manager.lock().add_item(&selected_dest, item)?;
            }
            self.add_prompt_state = None;
            self.status_message = Some(format!("Added {} items to '{}'", count, selected_dest));
            return Ok(());
        }

        let item = self.extract_selected_playlist_item(client.clone())?;
        playlist_manager.lock().add_item(&selected_dest, item)?;
        self.add_prompt_state = None;
        self.status_message = Some(format!("Added to '{}'", selected_dest));
        Ok(())
    }

    fn extract_selected_playlist_item(
        &self,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<PlaylistItem> {
        let idx = self
            .list_state
            .selected()
            .ok_or_else(|| anyhow::anyhow!("No item selected"))?;

        if let LibraryTab::Favorites = self.current_tab {
            if let NavigationLevel::Episodes { bvid, .. } = &self.nav_level {
                if let Some(episode) = self.episodes.get(idx) {
                    let artist = self
                        .current_video_info
                        .as_ref()
                        .map(|v| v.owner.name.clone())
                        .unwrap_or_default();
                    return Ok(PlaylistItem {
                        bvid: bvid.clone(),
                        cid: episode.cid,
                        title: episode.part.clone(),
                        artist,
                        duration: episode.duration,
                    });
                }
            }
        }

        let (bvid, title, artist, duration) = match self.current_tab {
            LibraryTab::Favorites => match &self.nav_level {
                NavigationLevel::Videos { .. } => self
                    .resources
                    .get(idx)
                    .map(|r| {
                        (
                            r.bvid.clone(),
                            r.title.clone(),
                            r.upper.name.clone(),
                            r.duration,
                        )
                    })
                    .ok_or_else(|| anyhow::anyhow!("No resource at index"))?,
                _ => anyhow::bail!("Navigate into a folder first"),
            },
            LibraryTab::WatchLater => self
                .watch_later
                .get(idx)
                .map(|w| {
                    (
                        w.bvid.clone(),
                        w.title.clone(),
                        w.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                        w.duration,
                    )
                })
                .ok_or_else(|| anyhow::anyhow!("No item at index"))?,
            LibraryTab::History => {
                let h = self
                    .history
                    .get(idx)
                    .ok_or_else(|| anyhow::anyhow!("No item at index"))?;
                let bvid = h
                    .bvid
                    .clone()
                    .ok_or_else(|| anyhow::anyhow!("History item has no bvid"))?;
                (
                    bvid,
                    h.title.clone(),
                    h.owner.as_ref().map(|o| o.name.clone()).unwrap_or_default(),
                    h.duration,
                )
            }
            _ => anyhow::bail!("Cannot add from this tab"),
        };

        let cid = {
            let client = client.lock();
            let rt = tokio::runtime::Runtime::new()?;
            let video_info = rt.block_on(client.get_video_info(&bvid))?;
            video_info.cid
        };

        Ok(PlaylistItem {
            bvid,
            cid,
            title,
            artist,
            duration,
        })
    }

    pub fn handle_add_prompt_key(
        &mut self,
        key: crossterm::event::KeyCode,
        playing_list: Arc<Mutex<PlayingListManager>>,
        playlist_manager: Arc<Mutex<PlaylistManager>>,
        client: Arc<Mutex<BilibiliClient>>,
    ) -> anyhow::Result<bool> {
        let prompt = match &mut self.add_prompt_state {
            Some(p) if p.is_active => p,
            _ => return Ok(false),
        };

        match key {
            KeyCode::Char('j') | KeyCode::Down => {
                if prompt.selected + 1 < prompt.destinations.len() {
                    prompt.selected += 1;
                }
                Ok(true)
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if prompt.selected > 0 {
                    prompt.selected -= 1;
                }
                Ok(true)
            }
            KeyCode::Enter => {
                self.resolve_add_target(playing_list, playlist_manager, client)?;
                Ok(true)
            }
            KeyCode::Esc => {
                self.add_prompt_state = None;
                Ok(true)
            }
            _ => Ok(true),
        }
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
