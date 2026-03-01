use anyhow::{Context, Result};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use parking_lot::Mutex;
use crate::storage::{Config, OutputFormat};
use super::extractor::AudioExtractor;

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub id: u64,
    pub bvid: String,
    pub title: String,
    pub url: String,
    pub status: DownloadStatus,
    pub progress: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadStatus {
    Pending,
    Downloading { bytes_done: u64, total: u64 },
    Extracting,
    Completed,
    Failed(String),
}

pub struct DownloadManager {
    queue: Arc<Mutex<VecDeque<DownloadTask>>>,
    config: Config,
    next_id: Arc<Mutex<u64>>,
}

impl DownloadManager {
    pub fn new(config: Config) -> Self {
        Self {
            queue: Arc::new(Mutex::new(VecDeque::new())),
            config,
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn add(&self, bvid: String, title: String, url: String) -> u64 {
        let mut id = self.next_id.lock();
        let task_id = *id;
        *id += 1;

        let task = DownloadTask {
            id: task_id,
            bvid,
            title,
            url,
            status: DownloadStatus::Pending,
            progress: 0.0,
        };

        self.queue.lock().push_back(task);
        task_id
    }

    pub fn get_queue(&self) -> Vec<DownloadTask> {
        self.queue.lock().iter().cloned().collect()
    }

    pub async fn download_next(&self) -> Result<()> {
        let task = {
            let mut queue = self.queue.lock();
            queue.front_mut().map(|t| {
                t.status = DownloadStatus::Downloading { bytes_done: 0, total: 0 };
                t.clone()
            })
        };

        if let Some(task) = task {
            let response = reqwest::get(&task.url).await?;
            let bytes = response.bytes().await?;

            let temp_path = self.config.download_dir.join(format!("{}.temp", task.bvid));
            std::fs::create_dir_all(&self.config.download_dir)?;
            std::fs::write(&temp_path, &bytes)?;

            let output_path = self.get_output_path(&task.bvid, &task.title);
            AudioExtractor::extract(&temp_path, &output_path, &self.config.output_format)?;
            std::fs::remove_file(&temp_path)?;

            let mut queue = self.queue.lock();
            if let Some(front) = queue.front_mut() {
                front.status = DownloadStatus::Completed;
                front.progress = 100.0;
            }
        }

        Ok(())
    }

    fn get_output_path(&self, bvid: &str, title: &str) -> PathBuf {
        let ext = match self.config.output_format {
            OutputFormat::Flac => "flac",
            OutputFormat::Mp3 { .. } => "mp3",
            OutputFormat::Opus { .. } => "opus",
        };

        let safe_title: String = title
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { '_' })
            .collect();

        self.config.download_dir.join(format!("{} - {}.{}", bvid, safe_title, ext))
    }
}
