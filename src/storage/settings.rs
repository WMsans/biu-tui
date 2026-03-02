use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    LoopOne,
    NoLoop,
    LoopList,
}

impl LoopMode {
    pub fn next(self) -> Self {
        match self {
            LoopMode::LoopOne => LoopMode::NoLoop,
            LoopMode::NoLoop => LoopMode::LoopList,
            LoopMode::LoopList => LoopMode::LoopOne,
        }
    }

    pub fn prev(self) -> Self {
        match self {
            LoopMode::LoopOne => LoopMode::LoopList,
            LoopMode::NoLoop => LoopMode::LoopOne,
            LoopMode::LoopList => LoopMode::NoLoop,
        }
    }

    pub fn display_name(&self) -> &str {
        match self {
            LoopMode::LoopOne => "Loop One",
            LoopMode::NoLoop => "No Loop",
            LoopMode::LoopList => "Loop List",
        }
    }
}

impl Default for LoopMode {
    fn default() -> Self {
        LoopMode::NoLoop
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub volume: u32,
    pub loop_mode: LoopMode,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            volume: 100,
            loop_mode: LoopMode::default(),
        }
    }
}

impl Settings {
    pub fn settings_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    pub fn load() -> Result<Self> {
        let path = Self::settings_dir()?.join("settings.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            let settings = Self::default();
            settings.save()?;
            Ok(settings)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::settings_dir()?.join("settings.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    pub fn volume_up(&mut self) {
        self.volume = (self.volume + 5).min(100);
        let _ = self.save();
    }

    pub fn volume_down(&mut self) {
        self.volume = self.volume.saturating_sub(5);
        let _ = self.save();
    }

    pub fn next_loop_mode(&mut self) {
        self.loop_mode = self.loop_mode.next();
        let _ = self.save();
    }

    pub fn prev_loop_mode(&mut self) {
        self.loop_mode = self.loop_mode.prev();
        let _ = self.save();
    }

    pub fn volume_float(&self) -> f32 {
        self.volume as f32 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_loop_mode_next_sequence() {
        assert_eq!(LoopMode::LoopOne.next(), LoopMode::NoLoop);
        assert_eq!(LoopMode::NoLoop.next(), LoopMode::LoopList);
        assert_eq!(LoopMode::LoopList.next(), LoopMode::LoopOne);
    }

    #[test]
    fn test_loop_mode_prev_sequence() {
        assert_eq!(LoopMode::LoopOne.prev(), LoopMode::LoopList);
        assert_eq!(LoopMode::NoLoop.prev(), LoopMode::LoopOne);
        assert_eq!(LoopMode::LoopList.prev(), LoopMode::NoLoop);
    }
}
