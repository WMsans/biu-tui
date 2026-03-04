use crate::api::types::*;
use crate::api::{HistoryItem, UserInfo, WatchLaterItem};
use anyhow::{Context, Result};
use reqwest::{cookie::Jar, Client};
use std::sync::Arc;

const BILIBILI_BASE_URL: &str = "https://api.bilibili.com";

pub struct BilibiliClient {
    pub(super) client: Client,
    cookie_jar: Arc<Jar>,
    csrf: Option<String>,
    pub mid: Option<u64>,
}

impl BilibiliClient {
    pub fn new() -> Result<Self> {
        let cookie_jar = Arc::new(Jar::default());
        let client = Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36")
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            cookie_jar,
            csrf: None,
            mid: None,
        })
    }

    pub fn load_cookies(&mut self, cookies: &str) -> Result<()> {
        let urls = vec![
            "https://www.bilibili.com"
                .parse()
                .context("Failed to parse URL")?,
            "https://api.bilibili.com"
                .parse()
                .context("Failed to parse URL")?,
            "https://passport.bilibili.com"
                .parse()
                .context("Failed to parse URL")?,
        ];

        // Split cookies and add each one individually to all relevant domains
        for cookie in cookies.split(';') {
            let cookie = cookie.trim();
            if !cookie.is_empty() {
                for url in &urls {
                    self.cookie_jar.add_cookie_str(cookie, url);
                }
            }
        }

        if let Some(csrf) = cookies
            .split(';')
            .find(|s| s.trim().starts_with("bili_jct="))
            .and_then(|s| s.split('=').nth(1))
            .map(|s| s.trim().to_string())
        {
            self.csrf = Some(csrf);
        }

        if let Some(mid) = cookies
            .split(';')
            .find(|s| s.trim().starts_with("DedeUserID="))
            .and_then(|s| s.split('=').nth(1))
            .and_then(|s| s.trim().parse::<u64>().ok())
        {
            self.mid = Some(mid);
        }

        Ok(())
    }

    pub fn set_csrf(&mut self, csrf: String) {
        self.csrf = Some(csrf);
    }

    pub fn set_mid(&mut self, mid: u64) {
        self.mid = Some(mid);
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let response = self.client.get(&url).send().await?;
        let response_text = response
            .text()
            .await
            .context("Failed to read response text")?;

        serde_json::from_str(&response_text).with_context(|| {
            format!(
                "Failed to parse API response from {}: {}",
                path, response_text
            )
        })
    }

    pub async fn post<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        form: &[(&str, &str)],
    ) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let mut form_data = form.to_vec();
        if let Some(ref csrf) = self.csrf {
            form_data.push(("csrf", csrf.as_str()));
        }
        let response = self.client.post(&url).form(&form_data).send().await?;
        let response_text = response
            .text()
            .await
            .context("Failed to read response text")?;

        serde_json::from_str(&response_text).with_context(|| {
            format!(
                "Failed to parse API response from {}: {}",
                path, response_text
            )
        })
    }

    pub async fn get_user_info(&self) -> Result<UserInfo> {
        let response: ApiResponse<UserInfo> = self.get("/x/space/myinfo").await?;
        response.data.context("Failed to get user info")
    }

    pub async fn get_video_info(&self, bvid: &str) -> Result<VideoInfo> {
        let path = format!("/x/web-interface/view?bvid={}", bvid);
        let response: ApiResponse<VideoInfo> = self.get(&path).await?;
        response.data.context("Failed to get video info")
    }

    /// Searches for resources within a favorite folder by keyword.
    ///
    /// # Arguments
    /// * `media_id` - The ID of the favorite folder to search in
    /// * `keyword` - The search keyword to filter resources
    ///
    /// # Returns
    /// A vector of favorite resources matching the keyword
    pub async fn search_folder_resources(
        &self,
        media_id: u64,
        keyword: &str,
    ) -> Result<Vec<FavoriteResource>> {
        let path = format!(
            "/x/v3/fav/resource/list?media_id={}&keyword={}&ps=100&pn=1",
            media_id,
            urlencoding::encode(keyword)
        );
        let response: ApiResponse<FavoriteResourceListData> = self
            .get(&path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search folder resources: {}", e))?;
        Ok(response.data.map(|d| d.medias).unwrap_or_default())
    }

    /// Searches for videos in the watch later list by keyword.
    ///
    /// # Arguments
    /// * `keyword` - The search keyword to filter watch later items
    ///
    /// # Returns
    /// A vector of watch later items matching the keyword
    pub async fn search_watch_later(&self, keyword: &str) -> Result<Vec<WatchLaterItem>> {
        let path = format!(
            "/x/v2/history/toview/web?key={}&ps=100&pn=1",
            urlencoding::encode(keyword)
        );
        let response: ApiResponse<WatchLaterListData> = self
            .get(&path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search watch later: {}", e))?;
        Ok(response.data.map(|d| d.list).unwrap_or_default())
    }

    /// Searches for videos in the viewing history by keyword.
    ///
    /// # Arguments
    /// * `keyword` - The search keyword to filter history items
    ///
    /// # Returns
    /// A vector of history items matching the keyword
    pub async fn search_history(&self, keyword: &str) -> Result<Vec<HistoryItem>> {
        let path = format!(
            "/x/web-interface/history/search?keyword={}&ps=100&pn=1",
            urlencoding::encode(keyword)
        );
        let response: ApiResponse<HistorySearchData> = self
            .get(&path)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to search history: {}", e))?;
        Ok(response.data.map(|d| d.list).unwrap_or_default())
    }
}
