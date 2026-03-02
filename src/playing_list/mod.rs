use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistItem {
    pub bvid: String,
    pub cid: u64,
    pub title: String,
    pub artist: String,
    pub duration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlayingListData {
    items: Vec<PlaylistItem>,
    current_index: Option<usize>,
}
