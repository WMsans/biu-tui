use anyhow::{Context, Result};
use reqwest::{cookie::Jar, Client};
use std::sync::Arc;
use crate::api::types::*;
use crate::api::UserInfo;

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
            "https://www.bilibili.com".parse().context("Failed to parse URL")?,
            "https://api.bilibili.com".parse().context("Failed to parse URL")?,
            "https://passport.bilibili.com".parse().context("Failed to parse URL")?,
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
        let response_text = response.text().await
            .context("Failed to read response text")?;
        
        serde_json::from_str(&response_text)
            .with_context(|| format!("Failed to parse API response from {}: {}", path, response_text))
    }

    pub async fn post<T: serde::de::DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let mut form_data = form.to_vec();
        if let Some(ref csrf) = self.csrf {
            form_data.push(("csrf", csrf.as_str()));
        }
        let response = self.client.post(&url).form(&form_data).send().await?;
        let response_text = response.text().await
            .context("Failed to read response text")?;
        
        serde_json::from_str(&response_text)
            .with_context(|| format!("Failed to parse API response from {}: {}", path, response_text))
    }

    pub async fn get_user_info(&self) -> Result<UserInfo> {
        let response: ApiResponse<UserInfo> = self.get("/x/space/myinfo").await?;
        response.data.context("Failed to get user info")
    }
}
