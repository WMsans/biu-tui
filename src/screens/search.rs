use crate::api::{FavoriteFolder, FavoriteResource, HistoryItem, VideoPage, WatchLaterItem};
use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone)]
pub struct SearchState {
    pub query: String,
    pub is_active: bool,
    pub cursor_position: usize,
}

impl SearchState {
    pub fn new() -> Self {
        Self {
            query: String::new(),
            is_active: true,
            cursor_position: 0,
        }
    }

    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor_position = 0;
    }

    pub fn push_char(&mut self, c: char) {
        self.query.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    pub fn pop_char(&mut self) -> bool {
        if self.cursor_position > 0 {
            self.query.remove(self.cursor_position - 1);
            self.cursor_position -= 1;
            true
        } else {
            false
        }
    }

    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

pub trait Searchable {
    fn matches(&self, query: &str) -> bool;
}

impl Searchable for FavoriteFolder {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for FavoriteResource {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
            || self.upper.name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for VideoPage {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.part.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for WatchLaterItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let owner_name = self.owner.as_ref().map(|o| o.name.as_str()).unwrap_or("");
        self.title.to_lowercase().contains(&query_lower)
            || owner_name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for HistoryItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        let owner_name = self.owner.as_ref().map(|o| o.name.as_str()).unwrap_or("");
        self.title.to_lowercase().contains(&query_lower)
            || owner_name.to_lowercase().contains(&query_lower)
    }
}

impl Searchable for PlaylistItem {
    fn matches(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.title.to_lowercase().contains(&query_lower)
            || self.artist.to_lowercase().contains(&query_lower)
    }
}
