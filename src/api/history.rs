use crate::api::{ApiResponse, BilibiliClient};
use anyhow::Result;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct HistoryItem {
    pub aid: u64,
    pub bvid: Option<String>,
    pub title: String,
    #[serde(rename = "pic")]
    pub cover: Option<String>,
    pub duration: u32,
    pub owner: Option<Owner>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct WatchLaterItem {
    pub bvid: String,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub owner: Option<Owner>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Owner {
    pub mid: u64,
    pub name: String,
}

impl BilibiliClient {
    pub async fn get_history(&self, page: u32) -> Result<Vec<HistoryItem>> {
        let path = format!("/x/v2/history?ps=20&pn={}", page);
        let response: ApiResponse<Vec<HistoryItem>> = self
            .get(&path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get history: {}", e))?;

        Ok(response.data.unwrap_or_default())
    }

    pub async fn get_watch_later(&self) -> Result<Vec<WatchLaterItem>> {
        let path = "/x/v2/history/toview";
        let response: ApiResponse<serde_json::Value> = self
            .get(path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get watch later: {}", e))?;

        let items = response
            .data
            .and_then(|d| d.get("list").cloned())
            .and_then(|l| serde_json::from_value(l).ok())
            .unwrap_or_default();

        Ok(items)
    }
}
