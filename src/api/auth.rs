use crate::api::BilibiliClient;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrCodeData {
    pub url: String,
    pub qrcode_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QrPollData {
    pub code: i32,
    pub message: String,
    pub url: Option<String>,
    pub refresh_token: Option<String>,
    pub timestamp: Option<u64>,
}

impl BilibiliClient {
    pub async fn generate_qrcode(&self) -> Result<QrCodeData> {
        let response = self
            .client
            .get("https://passport.bilibili.com/x/passport-login/web/qrcode/generate")
            .send()
            .await
            .context("Failed to request QR code")?;

        let json: serde_json::Value = response.json().await?;
        let data = json.get("data").context("No data in QR response")?;

        Ok(QrCodeData {
            url: data
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            qrcode_key: data
                .get("qrcode_key")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
        })
    }

    pub async fn poll_qrcode(&self, qrcode_key: &str) -> Result<QrPollData> {
        let url = format!(
            "https://passport.bilibili.com/x/passport-login/web/qrcode/poll?qrcode_key={}",
            qrcode_key
        );
        let response = self
            .client
            .get(&url)
            .send()
            .await
            .context("Failed to poll QR")?;
        let json: serde_json::Value = response.json().await?;

        Ok(QrPollData {
            code: json
                .get("data")
                .and_then(|d| d.get("code"))
                .and_then(|c| c.as_i64())
                .unwrap_or(-1) as i32,
            message: json
                .get("data")
                .and_then(|d| d.get("message"))
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string(),
            url: json
                .get("data")
                .and_then(|d| d.get("url"))
                .and_then(|u| u.as_str())
                .map(|s| s.to_string()),
            refresh_token: json
                .get("data")
                .and_then(|d| d.get("refresh_token"))
                .and_then(|r| r.as_str())
                .map(|s| s.to_string()),
            timestamp: json
                .get("data")
                .and_then(|d| d.get("timestamp"))
                .and_then(|t| t.as_u64()),
        })
    }
}
