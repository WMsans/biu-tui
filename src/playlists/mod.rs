use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::playing_list::PlaylistItem;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub items: Vec<PlaylistItem>,
}

pub struct PlaylistManager {
    base_dir: PathBuf,
}

impl PlaylistManager {
    pub fn new() -> Result<Self> {
        let base_dir = crate::storage::Settings::settings_dir()?.join("playlists");
        std::fs::create_dir_all(&base_dir).context("Failed to create playlists directory")?;

        let manager = Self { base_dir };
        if !manager.index_path().exists() {
            manager.rebuild_index()?;
        }
        Ok(manager)
    }

    fn index_path(&self) -> PathBuf {
        self.base_dir.join(".index.json")
    }

    fn playlist_path(&self, name: &str) -> PathBuf {
        let sanitized: String = name
            .chars()
            .map(|c| {
                if c == '/'
                    || c == '\\'
                    || c == ':'
                    || c == '*'
                    || c == '?'
                    || c == '"'
                    || c == '<'
                    || c == '>'
                    || c == '|'
                {
                    '_'
                } else {
                    c
                }
            })
            .collect();
        self.base_dir.join(format!("{}.json", sanitized))
    }

    fn load_index(&self) -> Result<Vec<String>> {
        let path = self.index_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path).context("Failed to read index file")?;
        let names: Vec<String> =
            serde_json::from_str(&content).context("Failed to parse index file")?;
        Ok(names)
    }

    fn save_index(&self, names: &[String]) -> Result<()> {
        let path = self.index_path();
        let content = serde_json::to_string_pretty(names).context("Failed to serialize index")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, content).context("Failed to write index")?;
        std::fs::rename(&temp, &path).context("Failed to save index")?;
        Ok(())
    }

    fn rebuild_index(&self) -> Result<()> {
        let mut names = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&self.base_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".json") && !name.starts_with('.') {
                    let without_ext = &name[..name.len() - 5];
                    names.push(without_ext.to_string());
                }
            }
        }
        names.sort();
        self.save_index(&names)
    }

    pub fn list_names(&self) -> Result<Vec<String>> {
        if !self.index_path().exists() {
            self.rebuild_index()?;
        }
        let names = self.load_index()?;
        let mut valid = Vec::new();
        let mut needs_fix = false;
        for name in &names {
            if self.playlist_path(name).exists() {
                valid.push(name.clone());
            } else {
                needs_fix = true;
            }
        }
        if needs_fix {
            self.save_index(&valid)?;
        }
        Ok(valid)
    }

    pub fn load(&self, name: &str) -> Result<Playlist> {
        let path = self.playlist_path(name);
        if !path.exists() {
            return Ok(Playlist {
                name: name.to_string(),
                items: Vec::new(),
            });
        }
        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read playlist file: {}", name))?;
        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse playlist: {}", name))
    }

    fn save(&self, playlist: &Playlist) -> Result<()> {
        let path = self.playlist_path(&playlist.name);
        let content =
            serde_json::to_string_pretty(playlist).context("Failed to serialize playlist")?;
        let temp = path.with_extension("json.tmp");
        std::fs::write(&temp, content).context("Failed to write playlist")?;
        std::fs::rename(&temp, &path).context("Failed to save playlist")?;
        Ok(())
    }

    pub fn create(&self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            anyhow::bail!("Playlist name cannot be empty");
        }
        if name.len() > 64 {
            anyhow::bail!("Playlist name too long (max 64 characters)");
        }
        let path = self.playlist_path(name);
        if path.exists() {
            anyhow::bail!("Playlist '{}' already exists", name);
        }
        let playlist = Playlist {
            name: name.to_string(),
            items: Vec::new(),
        };
        self.save(&playlist)?;
        let mut names = self.load_index()?;
        names.push(name.to_string());
        names.sort();
        self.save_index(&names)?;
        Ok(())
    }

    pub fn delete(&self, name: &str) -> Result<()> {
        let path = self.playlist_path(name);
        if path.exists() {
            std::fs::remove_file(&path)
                .with_context(|| format!("Failed to delete playlist file: {}", name))?;
        }
        let mut names = self.load_index()?;
        names.retain(|n| n != name);
        self.save_index(&names)?;
        Ok(())
    }

    pub fn rename(&self, old_name: &str, new_name: &str) -> Result<()> {
        let new_name = new_name.trim();
        if new_name.is_empty() {
            anyhow::bail!("Playlist name cannot be empty");
        }
        if new_name.len() > 64 {
            anyhow::bail!("Playlist name too long (max 64 characters)");
        }
        if self.playlist_path(new_name).exists() {
            anyhow::bail!("Playlist '{}' already exists", new_name);
        }
        let old_path = self.playlist_path(old_name);
        if !old_path.exists() {
            anyhow::bail!("Playlist '{}' does not exist", old_name);
        }

        let mut playlist = self.load(old_name)?;
        playlist.name = new_name.to_string();
        let new_path = self.playlist_path(new_name);
        let content =
            serde_json::to_string_pretty(&playlist).context("Failed to serialize playlist")?;
        std::fs::write(&new_path, content).context("Failed to write renamed playlist")?;

        if old_path != new_path {
            std::fs::remove_file(&old_path).ok();
        }

        let mut names = self.load_index()?;
        if let Some(pos) = names.iter().position(|n| n == old_name) {
            names[pos] = new_name.to_string();
        }
        names.sort();
        self.save_index(&names)?;
        Ok(())
    }

    pub fn add_item(&self, playlist_name: &str, item: PlaylistItem) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        playlist.items.push(item);
        self.save(&playlist)?;
        Ok(())
    }

    pub fn remove_item(&self, playlist_name: &str, index: usize) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        if index < playlist.items.len() {
            playlist.items.remove(index);
            self.save(&playlist)?;
        }
        Ok(())
    }

    pub fn reorder(&self, playlist_name: &str, from: usize, to: usize) -> Result<()> {
        let mut playlist = self.load(playlist_name)?;
        let len = playlist.items.len();
        if from < len && to < len && from != to {
            let item = playlist.items.remove(from);
            playlist.items.insert(to, item);
            self.save(&playlist)?;
        }
        Ok(())
    }

    pub fn get_items(&self, playlist_name: &str) -> Result<Vec<PlaylistItem>> {
        let playlist = self.load(playlist_name)?;
        Ok(playlist.items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn create_test_item(id: usize) -> PlaylistItem {
        PlaylistItem {
            bvid: format!("BV{}", id),
            cid: id as u64,
            title: format!("Song {}", id),
            artist: format!("Artist {}", id),
            duration: 180,
        }
    }

    struct TestEnv {
        manager: PlaylistManager,
        _dir: tempfile::TempDir,
    }

    fn setup() -> TestEnv {
        let dir = tempdir().unwrap();
        let base_dir = dir.path().join("playlists");
        std::fs::create_dir_all(&base_dir).unwrap();
        let manager = PlaylistManager { base_dir };
        TestEnv { manager, _dir: dir }
    }

    #[test]
    fn test_create_and_list_playlists() {
        let env = setup();
        env.manager.create("Workout").unwrap();
        env.manager.create("Study").unwrap();
        let names = env.manager.list_names().unwrap();
        assert_eq!(names, vec!["Study", "Workout"]);
    }

    #[test]
    fn test_create_duplicate_rejected() {
        let env = setup();
        env.manager.create("Test").unwrap();
        assert!(env.manager.create("Test").is_err());
    }

    #[test]
    fn test_create_empty_name_rejected() {
        let env = setup();
        assert!(env.manager.create("").is_err());
        assert!(env.manager.create("   ").is_err());
    }

    #[test]
    fn test_create_name_too_long_rejected() {
        let env = setup();
        let long_name = "a".repeat(65);
        assert!(env.manager.create(&long_name).is_err());
    }

    #[test]
    fn test_delete_playlist() {
        let env = setup();
        env.manager.create("TempPlaylist").unwrap();
        env.manager.delete("TempPlaylist").unwrap();
        assert!(env.manager.list_names().unwrap().is_empty());
    }

    #[test]
    fn test_rename_playlist() {
        let env = setup();
        env.manager.create("OldName").unwrap();
        env.manager.rename("OldName", "NewName").unwrap();
        let names = env.manager.list_names().unwrap();
        assert_eq!(names, vec!["NewName"]);
        assert!(!env.manager.playlist_path("OldName").exists());
        assert!(env.manager.playlist_path("NewName").exists());
    }

    #[test]
    fn test_rename_to_existing_rejected() {
        let env = setup();
        env.manager.create("First").unwrap();
        env.manager.create("Second").unwrap();
        assert!(env.manager.rename("First", "Second").is_err());
    }

    #[test]
    fn test_add_and_get_items() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();
        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Song 1");
    }

    #[test]
    fn test_remove_item() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();
        env.manager.add_item("Songs", create_test_item(3)).unwrap();
        env.manager.remove_item("Songs", 1).unwrap();
        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Song 1");
        assert_eq!(items[1].title, "Song 3");
    }

    #[test]
    fn test_reorder_items() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.add_item("Songs", create_test_item(2)).unwrap();
        env.manager.add_item("Songs", create_test_item(3)).unwrap();
        env.manager.reorder("Songs", 0, 2).unwrap();
        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items[0].title, "Song 2");
        assert_eq!(items[1].title, "Song 3");
        assert_eq!(items[2].title, "Song 1");
    }

    #[test]
    fn test_reorder_noop_same_index() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.reorder("Songs", 0, 0).unwrap();
        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_reorder_out_of_bounds_noop() {
        let env = setup();
        env.manager.create("Songs").unwrap();
        env.manager.add_item("Songs", create_test_item(1)).unwrap();
        env.manager.reorder("Songs", 0, 99).unwrap();
        let items = env.manager.get_items("Songs").unwrap();
        assert_eq!(items.len(), 1);
    }

    #[test]
    fn test_persistence_roundtrip() {
        let env = setup();
        env.manager.create("Persist").unwrap();
        env.manager
            .add_item("Persist", create_test_item(1))
            .unwrap();
        env.manager
            .add_item("Persist", create_test_item(2))
            .unwrap();
        let manager2 = PlaylistManager {
            base_dir: env.manager.base_dir.clone(),
        };
        let items = manager2.get_items("Persist").unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(manager2.list_names().unwrap(), vec!["Persist"]);
    }

    #[test]
    fn test_empty_playlist() {
        let env = setup();
        env.manager.create("Empty").unwrap();
        let items = env.manager.get_items("Empty").unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_index_rebuild_from_files() {
        let env = setup();
        env.manager.create("Alpha").unwrap();
        env.manager.create("Beta").unwrap();
        std::fs::remove_file(env.manager.index_path()).unwrap();
        let manager2 = PlaylistManager {
            base_dir: env.manager.base_dir.clone(),
        };
        let names = manager2.list_names().unwrap();
        assert_eq!(names, vec!["Alpha", "Beta"]);
    }
}
