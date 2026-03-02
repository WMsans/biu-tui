use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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

pub struct PlayingListManager {
    items: Vec<PlaylistItem>,
    current_index: Option<usize>,
    storage_path: PathBuf,
}

impl PlayingListManager {
    pub fn new() -> Result<Self> {
        let storage_path = crate::storage::Settings::settings_dir()?.join("playing_list.json");

        let mut manager = Self {
            items: Vec::new(),
            current_index: None,
            storage_path,
        };

        manager.load()?;
        Ok(manager)
    }

    pub fn new_empty() -> Result<Self> {
        let storage_path = crate::storage::Settings::settings_dir()?.join("playing_list.json");

        Ok(Self {
            items: Vec::new(),
            current_index: None,
            storage_path,
        })
    }

    fn load(&mut self) -> Result<()> {
        if !self.storage_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&self.storage_path)
            .context("Failed to read playing list file")?;

        if content.trim().is_empty() {
            return Ok(());
        }

        let data: PlayingListData =
            serde_json::from_str(&content).context("Failed to parse playing list")?;

        self.items = data.items;
        self.current_index = data.current_index;

        if let Some(idx) = self.current_index {
            if idx >= self.items.len() {
                self.current_index = None;
            }
        }

        Ok(())
    }

    fn save(&self) -> Result<()> {
        let data = PlayingListData {
            items: self.items.clone(),
            current_index: self.current_index,
        };

        let content =
            serde_json::to_string_pretty(&data).context("Failed to serialize playing list")?;

        let temp_path = self.storage_path.with_extension("json.tmp");
        std::fs::write(&temp_path, content).context("Failed to write playing list")?;

        std::fs::rename(&temp_path, &self.storage_path).context("Failed to save playing list")?;

        Ok(())
    }

    pub fn add(&mut self, item: PlaylistItem) {
        self.items.push(item);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
        let _ = self.save();
    }

    pub fn add_all(&mut self, items: Vec<PlaylistItem>) {
        let was_empty = self.items.is_empty();
        self.items.extend(items);
        if was_empty && !self.items.is_empty() {
            self.current_index = Some(0);
        }
        let _ = self.save();
    }

    pub fn remove(&mut self, index: usize) -> Option<PlaylistItem> {
        if index >= self.items.len() {
            return None;
        }

        let item = self.items.remove(index);

        if let Some(current) = self.current_index {
            if current == index {
                if self.items.is_empty() {
                    self.current_index = None;
                } else if current >= self.items.len() {
                    self.current_index = Some(self.items.len() - 1);
                }
            } else if current > index {
                self.current_index = Some(current - 1);
            }
        }

        let _ = self.save();
        Some(item)
    }

    pub fn jump_to(&mut self, index: usize) {
        if index < self.items.len() {
            self.current_index = Some(index);
            let _ = self.save();
        }
    }

    pub fn current(&self) -> Option<&PlaylistItem> {
        self.current_index.and_then(|idx| self.items.get(idx))
    }

    pub fn next(&mut self) -> Option<&PlaylistItem> {
        let current = self.current_index?;

        if current + 1 < self.items.len() {
            self.current_index = Some(current + 1);
        } else {
            self.current_index = Some(0);
        }

        let _ = self.save();
        self.current()
    }

    pub fn items(&self) -> &[PlaylistItem] {
        &self.items
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.current_index = None;
        let _ = self.save();
    }
}
