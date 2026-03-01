use anyhow::{Context, Result};
use std::path::PathBuf;

pub struct CookieStorage;

impl CookieStorage {
    pub fn cookie_path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .context("Cannot determine config directory")?
            .join("biu-tui");
        std::fs::create_dir_all(&dir).context("Failed to create config directory")?;
        Ok(dir.join("cookies.json"))
    }

    pub fn save(cookies: &str) -> Result<()> {
        let path = Self::cookie_path()?;
        std::fs::write(&path, cookies)?;
        Ok(())
    }

    pub fn load() -> Result<Option<String>> {
        let path = Self::cookie_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(Some(content))
        } else {
            Ok(None)
        }
    }

    pub fn clear() -> Result<()> {
        let path = Self::cookie_path()?;
        if path.exists() {
            std::fs::remove_file(&path)?;
        }
        Ok(())
    }
}
