use crate::api::{BilibiliClient, HistoryItem, WatchLaterItem};
use crate::api::{FavoriteFolder, FavoriteResource};
use crate::audio::AudioPlayer;
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum NavigationLevel {
    Folders,
    Videos { folder_id: u64, folder_title: String },
    Episodes { folder_id: u64, folder_id_title: String, bvid: String, video_title: String },
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
        }
    }

    pub fn set_loop_mode(&mut self, mode: LoopMode) {
        self.loop_mode = mode;
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
            LoopMode::LoopFolder => {
                let next_idx = if current_idx + 1 < self.resources.len() {
                    current_idx + 1
                } else {
                    0
                };
                Some(NextAction::PlayNext(next_idx))
            }
        }
    }

    pub async fn load_data(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
        let (folders, watch_later, history) = {
            let client = client.lock();
            let mid = client.mid;
            
            let folders_result = async {
                if let Some(mid) = mid {
                    client.get_created_folders(mid).await
                        .map_err(|e| anyhow::anyhow!("Favorites API failed: {}", e))
                } else {
                    Ok(Vec::new())
                }
            };
            
            let watch_later_result = async {
                client.get_watch_later().await
                    .map_err(|e| anyhow::anyhow!("Watch Later API failed: {}", e))
            };
            
            let history_result = async {
                client.get_history(1).await
                    .map_err(|e| anyhow::anyhow!("History API failed: {}", e))
            };
            
            let (folders, watch_later, history) = tokio::try_join!(
                folders_result,
                watch_later_result,
                history_result
            )?;
            
            (folders, watch_later, history)
        };

        self.folders = folders;
        self.watch_later = watch_later;
        self.history = history;
        
        Ok(())
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect, player: Option<&AudioPlayer>) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(2),  // now playing: 1 for border + 1 for content
                Constraint::Length(2),  // progress bar: 1 for border + 1 for content
                Constraint::Length(2),  // help bar: 1 for border + 1 for content
            ])
            .split(area);

        let titles: Vec<&str> = vec!["Favorites", "Watch Later", "History"];
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
            NavigationLevel::Episodes { folder_id_title, video_title, .. } => {
                format!("Favorites > {} > {}", folder_id_title, video_title)
            }
        };
        let breadcrumb = Paragraph::new(breadcrumb_text)
            .style(Style::default().fg(Color::Yellow));
        f.render_widget(breadcrumb, chunks[1]);

        let items: Vec<ListItem> = match self.current_tab {
            LibraryTab::Favorites => {
                match &self.nav_level {
                    NavigationLevel::Folders => {
                        self.folders
                            .iter()
                            .map(|f| ListItem::new(format!("{} ({})", f.title, f.media_count)))
                            .collect()
                    }
                    NavigationLevel::Videos { .. } => {
                        self.resources
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
                            .collect()
                    }
                    NavigationLevel::Episodes { .. } => {
                        self.episodes
                            .iter()
                            .map(|ep| {
                                ListItem::new(format!(
                                    "P{} {}  {}",
                                    ep.page,
                                    ep.part,
                                    format_duration(ep.duration),
                                ))
                            })
                            .collect()
                    }
                }
            }
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

        let progress_text = if let Some(p) = player {
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
                let filled_chars: String = std::iter::repeat('━').take(filled).collect();
                let empty_chars: String = std::iter::repeat('─').take(bar_width - filled).collect();
                format!("{}╾{}", filled_chars, empty_chars)
            } else {
                String::new()
            };
            
            format!("{}  {} / {}", bar, pos_str, dur_str)
        } else {
            "━━──────────────  --:-- / --:--".to_string()
        };

        let progress_bar = Paragraph::new(progress_text)
            .style(Style::default().fg(Color::Cyan))
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(progress_bar, chunks[4]);

        let help = Paragraph::new("[j/k] Navigate  [Enter] Select  [Esc] Back  [s] Settings  [Tab] Switch")
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[5]);
    }

    pub fn next_item(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            let i = self
                .list_state
                .selected()
                .map_or(0, |i| if i >= len - 1 { 0 } else { i + 1 });
            self.list_state.select(Some(i));
        }
    }

    pub fn prev_item(&mut self) {
        let len = self.current_list_len();
        if len > 0 {
            let i = self
                .list_state
                .selected()
                .map_or(0, |i| if i == 0 { len - 1 } else { i - 1 });
            self.list_state.select(Some(i));
        }
    }

    fn current_list_len(&self) -> usize {
        match self.current_tab {
            LibraryTab::Favorites => {
                match &self.nav_level {
                    NavigationLevel::Folders => self.folders.len(),
                    NavigationLevel::Videos { .. } => self.resources.len(),
                    NavigationLevel::Episodes { .. } => self.episodes.len(),
                }
            }
            LibraryTab::WatchLater => self.watch_later.len(),
            LibraryTab::History => self.history.len(),
        }
    }

    pub fn handle_enter(
        &mut self,
        client: Arc<Mutex<BilibiliClient>>,
        player: &mut Option<AudioPlayer>,
    ) -> anyhow::Result<()> {
        match self.current_tab {
            LibraryTab::Favorites => {
                match &self.nav_level {
                    NavigationLevel::Folders => {
                        self.select_folder(client)?;
                    }
                    NavigationLevel::Videos { folder_id, folder_title } => {
                        let folder_id = *folder_id;
                        let folder_title = folder_title.clone();
                        self.select_video_or_episodes(client, player, folder_id, folder_title)?;
                    }
                    NavigationLevel::Episodes { bvid, .. } => {
                        let bvid = bvid.clone();
                        self.play_episode(client, player, &bvid)?;
                    }
                }
            }
            LibraryTab::WatchLater | LibraryTab::History => {
                self.play_selected(client, player)?;
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
                self.nav_level = NavigationLevel::Videos { folder_id, folder_title };
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

    pub fn go_back(&mut self) {
        match &self.nav_level {
            NavigationLevel::Videos { .. } => {
                self.nav_level = NavigationLevel::Folders;
                self.resources.clear();
                self.list_state.select(Some(0));
            }
            NavigationLevel::Episodes { folder_id, folder_id_title, .. } => {
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
