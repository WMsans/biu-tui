use anyhow::{Context, Result};
use reqwest::{cookie::Jar, Client};
use std::sync::Arc;
use crate::api::types::*;

const BILIBILI_BASE_URL: &str = "https://api.bilibili.com";

pub struct BilibiliClient {
    client: Client,
    cookie_jar: Arc<Jar>,
    csrf: Option<String>,
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
        })
    }

    pub fn set_csrf(&mut self, csrf: String) {
        self.csrf = Some(csrf);
    }

    pub async fn get<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let response = self.client.get(&url).send().await?;
        let api_response = response.json::<ApiResponse<T>>().await?;
        Ok(api_response)
    }

    pub async fn post<T: serde::de::DeserializeOwned>(&self, path: &str, form: &[(&str, &str)]) -> Result<ApiResponse<T>> {
        let url = format!("{}{}", BILIBILI_BASE_URL, path);
        let mut form_data = form.to_vec();
        if let Some(ref csrf) = self.csrf {
            form_data.push(("csrf", csrf.as_str()));
        }
        let response = self.client.post(&url).form(&form_data).send().await?;
        let api_response = response.json::<ApiResponse<T>>().await?;
        Ok(api_response)
    }
}
