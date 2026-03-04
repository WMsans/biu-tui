use crate::api::{FavoriteFolder, FavoriteResource, HistoryItem, VideoPage, WatchLaterItem};
use crate::playing_list::PlaylistItem;

/// Manages the state for a search input field in the TUI.
///
/// Tracks the search query string, cursor position (as byte index), and active state.
/// Supports UTF-8 characters correctly by tracking byte positions internally.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// The current search query string.
    pub query: String,
    /// Whether the search input is currently active/focused.
    pub is_active: bool,
    /// The byte index position of the cursor within the query.
    /// This is a byte offset, not a character index, to work correctly with String operations.
    pub cursor_position: usize,
}

impl SearchState {
    /// Creates a new SearchState with an empty query and active state.
    pub fn new() -> Self {
        Self {
            query: String::new(),
            is_active: true,
            cursor_position: 0,
        }
    }

    /// Clears the search query and resets the cursor position to 0.
    pub fn clear(&mut self) {
        self.query.clear();
        self.cursor_position = 0;
    }

    /// Inserts a character at the current cursor position and advances the cursor.
    ///
    /// The cursor position is advanced by the UTF-8 byte length of the character,
    /// ensuring correct positioning for multi-byte characters.
    pub fn push_char(&mut self, c: char) {
        self.query.insert(self.cursor_position, c);
        self.cursor_position += c.len_utf8();
    }

    /// Removes the character immediately before the cursor and moves the cursor back.
    ///
    /// Returns `true` if a character was removed, `false` if the cursor was at position 0.
    /// The cursor position is decremented by the UTF-8 byte length of the removed character,
    /// ensuring correct positioning for multi-byte characters.
    pub fn pop_char(&mut self) -> bool {
        if self.cursor_position == 0 {
            return false;
        }

        if let Some(c) = self.query[..self.cursor_position].chars().next_back() {
            let char_len = c.len_utf8();
            let char_start = self.cursor_position - char_len;
            self.query.remove(char_start);
            self.cursor_position -= char_len;
            true
        } else {
            false
        }
    }

    /// Returns `true` if the search query is empty.
    pub fn is_empty(&self) -> bool {
        self.query.is_empty()
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

/// A trait for types that can be matched against a search query.
///
/// Implementations typically perform case-insensitive substring matching
/// on relevant text fields of the type.
pub trait Searchable {
    /// Returns `true` if this item matches the given search query.
    ///
    /// The query is typically matched case-insensitively against
    /// relevant text fields of the implementing type.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_state_new() {
        let state = SearchState::new();
        assert!(state.query.is_empty());
        assert!(state.is_active);
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_search_state_default() {
        let state = SearchState::default();
        assert!(state.query.is_empty());
        assert!(state.is_active);
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_push_char_ascii() {
        let mut state = SearchState::new();
        state.push_char('a');
        assert_eq!(state.query, "a");
        assert_eq!(state.cursor_position, 1);

        state.push_char('b');
        assert_eq!(state.query, "ab");
        assert_eq!(state.cursor_position, 2);
    }

    #[test]
    fn test_push_char_utf8_two_byte() {
        let mut state = SearchState::new();
        state.push_char('é');
        assert_eq!(state.query, "é");
        assert_eq!(state.cursor_position, 2);

        state.push_char('ü');
        assert_eq!(state.query, "éü");
        assert_eq!(state.cursor_position, 4);
    }

    #[test]
    fn test_push_char_utf8_three_byte() {
        let mut state = SearchState::new();
        state.push_char('中');
        assert_eq!(state.query, "中");
        assert_eq!(state.cursor_position, 3);

        state.push_char('文');
        assert_eq!(state.query, "中文");
        assert_eq!(state.cursor_position, 6);
    }

    #[test]
    fn test_push_char_utf8_four_byte() {
        let mut state = SearchState::new();
        state.push_char('🎉');
        assert_eq!(state.query, "🎉");
        assert_eq!(state.cursor_position, 4);

        state.push_char('🚀');
        assert_eq!(state.query, "🎉🚀");
        assert_eq!(state.cursor_position, 8);
    }

    #[test]
    fn test_push_char_mixed() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('中');
        state.push_char('b');
        assert_eq!(state.query, "a中b");
        assert_eq!(state.cursor_position, 5);
    }

    #[test]
    fn test_pop_char_ascii() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('b');

        assert!(state.pop_char());
        assert_eq!(state.query, "a");
        assert_eq!(state.cursor_position, 1);

        assert!(state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);

        assert!(!state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_pop_char_utf8_two_byte() {
        let mut state = SearchState::new();
        state.push_char('é');
        state.push_char('ü');

        assert!(state.pop_char());
        assert_eq!(state.query, "é");
        assert_eq!(state.cursor_position, 2);

        assert!(state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_pop_char_utf8_three_byte() {
        let mut state = SearchState::new();
        state.push_char('中');
        state.push_char('文');

        assert!(state.pop_char());
        assert_eq!(state.query, "中");
        assert_eq!(state.cursor_position, 3);

        assert!(state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_pop_char_utf8_four_byte() {
        let mut state = SearchState::new();
        state.push_char('🎉');
        state.push_char('🚀');

        assert!(state.pop_char());
        assert_eq!(state.query, "🎉");
        assert_eq!(state.cursor_position, 4);

        assert!(state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_pop_char_mixed_utf8() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('中');
        state.push_char('🎉');
        state.push_char('b');

        assert!(state.pop_char());
        assert_eq!(state.query, "a中🎉");
        assert_eq!(state.cursor_position, 8);

        assert!(state.pop_char());
        assert_eq!(state.query, "a中");
        assert_eq!(state.cursor_position, 4);

        assert!(state.pop_char());
        assert_eq!(state.query, "a");
        assert_eq!(state.cursor_position, 1);

        assert!(state.pop_char());
        assert_eq!(state.query, "");
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_pop_char_empty() {
        let mut state = SearchState::new();
        assert!(!state.pop_char());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_clear() {
        let mut state = SearchState::new();
        state.push_char('a');
        state.push_char('b');
        state.clear();
        assert!(state.query.is_empty());
        assert_eq!(state.cursor_position, 0);
    }

    #[test]
    fn test_is_empty() {
        let mut state = SearchState::new();
        assert!(state.is_empty());
        state.push_char('a');
        assert!(!state.is_empty());
    }

    #[test]
    fn test_cursor_position_after_operations() {
        let mut state = SearchState::new();
        state.push_char('你');
        state.push_char('好');
        state.push_char('世');
        state.push_char('界');
        assert_eq!(state.query, "你好世界");
        assert_eq!(state.cursor_position, 12);

        state.pop_char();
        assert_eq!(state.query, "你好世");
        assert_eq!(state.cursor_position, 9);

        state.push_char('界');
        assert_eq!(state.query, "你好世界");
        assert_eq!(state.cursor_position, 12);
    }

    #[test]
    fn test_searchable_favorite_folder() {
        let folder = FavoriteFolder {
            id: 1,
            title: "My Music".to_string(),
            media_count: 10,
        };
        assert!(folder.matches("music"));
        assert!(folder.matches("MUSIC"));
        assert!(folder.matches("my"));
        assert!(!folder.matches("video"));
    }

    #[test]
    fn test_searchable_favorite_resource() {
        use crate::api::Upper;

        let resource = FavoriteResource {
            id: 1,
            bvid: "BV123".to_string(),
            title: "Test Song".to_string(),
            cover: None,
            duration: 180,
            upper: Upper {
                mid: 123,
                name: "Artist Name".to_string(),
            },
        };
        assert!(resource.matches("test"));
        assert!(resource.matches("ARTIST"));
        assert!(!resource.matches("other"));
    }

    #[test]
    fn test_searchable_playlist_item() {
        let item = PlaylistItem {
            bvid: "BV123".to_string(),
            cid: 456,
            title: "Song Title".to_string(),
            artist: "Artist Name".to_string(),
            duration: 180,
        };
        assert!(item.matches("song"));
        assert!(item.matches("ARTIST"));
        assert!(!item.matches("other"));
    }

    #[test]
    fn test_searchable_utf8_query() {
        let folder = FavoriteFolder {
            id: 1,
            title: "我的音乐".to_string(),
            media_count: 5,
        };
        assert!(folder.matches("音乐"));
        assert!(folder.matches("我"));
        assert!(!folder.matches("视频"));
    }

    #[test]
    fn test_searchable_empty_query() {
        let folder = FavoriteFolder {
            id: 1,
            title: "Music".to_string(),
            media_count: 0,
        };
        assert!(folder.matches(""));
    }
}
