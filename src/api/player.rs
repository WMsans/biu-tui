use anyhow::{Context, Result};
use crate::api::{BilibiliClient, ApiResponse, PlayUrlData, AudioDash};

#[derive(Debug, Clone)]
pub struct AudioStream {
    pub url: String,
    pub quality: AudioQuality,
    pub format: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum AudioQuality {
    K64 = 30216,
    K128 = 30232,
    K192 = 30280,
    HiRes = 30250,
    Dolby = 30251,
}

impl BilibiliClient {
    pub async fn get_playurl(&self, bvid: &str, cid: u64) -> Result<PlayUrlData> {
        let path = format!(
            "/x/player/wbi/playurl?bvid={}&cid={}&fnval=16&fnver=0&fourk=0",
            bvid, cid
        );
        let response: ApiResponse<PlayUrlData> = self.get(&path).await?;

        response.data.context("No playurl data in response")
    }

    pub async fn get_best_audio(&self, bvid: &str, cid: u64) -> Result<AudioStream> {
        let data = self.get_playurl(bvid, cid).await?;

        if let Some(dash) = data.dash {
            let mut best_audio: Option<&AudioDash> = None;
            for audio in &dash.audio {
                if best_audio.is_none() || audio.bandwidth > best_audio.unwrap().bandwidth {
                    best_audio = Some(audio);
                }
            }

            if let Some(audio) = best_audio {
                let url = audio.base_url.clone()
                    .or_else(|| audio.backup_url.as_ref().and_then(|v| v.first().cloned()))
                    .context("No audio URL found")?;

                let quality = match audio.id {
                    30250 => AudioQuality::HiRes,
                    30251 => AudioQuality::Dolby,
                    30280 => AudioQuality::K192,
                    30232 => AudioQuality::K128,
                    _ => AudioQuality::K64,
                };

                return Ok(AudioStream {
                    url,
                    quality,
                    format: if audio.codecid == 0 { "mp4a".to_string() } else { "flac".to_string() },
                });
            }
        }

        anyhow::bail!("No audio stream found")
    }
}
