use crate::api::{BilibiliClient, HistoryItem, WatchLaterItem};
use crate::api::{FavoriteFolder, FavoriteResource};
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

#[derive(Clone)]
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
                } else {
                    self.folders
                        .iter()
                        .map(|f| ListItem::new(format!("{} ({})", f.title, f.media_count)))
                        .collect()
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
        f.render_stateful_widget(list, chunks[1], &mut self.list_state);

        let help = Paragraph::new("[j/k] Navigate  [Enter] Select  [Esc] Back  [d] Download  [Tab] Switch")
            .block(Block::default().borders(Borders::TOP));
        f.render_widget(help, chunks[2]);
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

    pub fn select_folder(&mut self, client: Arc<Mutex<BilibiliClient>>) -> anyhow::Result<()> {
        if self.current_tab == LibraryTab::Favorites && self.selected_folder.is_none() {
            if let Some(idx) = self.list_state.selected() {
                if idx < self.folders.len() {
                    let folder_id = self.folders[idx].id;
                    self.selected_folder = Some(folder_id);
                    
                    let resources = {
                        let client = client.lock();
                        let rt = tokio::runtime::Runtime::new()?;
                        rt.block_on(client.get_folder_resources(folder_id, 1))?
                    };
                    
                    self.resources = resources.0;
                    self.list_state.select(Some(0));
                }
            }
        }
        Ok(())
    }

    pub fn go_back(&mut self) {
        if self.selected_folder.is_some() {
            self.selected_folder = None;
            self.resources.clear();
            self.list_state.select(Some(0));
        }
    }
}

fn format_duration(seconds: u32) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{}:{:02}", mins, secs)
}
