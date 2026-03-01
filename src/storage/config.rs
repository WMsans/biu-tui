use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub download_dir: PathBuf,
    pub output_format: OutputFormat,
    pub audio_quality: AudioQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum OutputFormat {
    Flac,
    Mp3 { bitrate: u32 },
    Opus { bitrate: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AudioQuality {
    HiRes,
    Flac,
    K192,
    K128,
}

impl Default for Config {
    fn default() -> Self {
        let download_dir = dirs::audio_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("biu-tui");

        Self {
            download_dir,
            output_format: OutputFormat::Flac,
            audio_quality: AudioQuality::Flac,
        }
    }
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir)
    }

    pub fn load() -> Result<Self> {
        let path = Self::config_dir()?.join("config.json");
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            let config = Self::default();
            config.save()?;
            Ok(config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_dir()?.join("config.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
