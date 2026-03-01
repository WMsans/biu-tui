use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub code: i32,
    pub message: Option<String>,
    pub data: Option<T>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub mid: u64,
    #[serde(rename = "name")]
    pub uname: String,
    pub face: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteFolder {
    pub id: u64,
    pub title: String,
    pub media_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FavoriteResource {
    pub id: u64,
    pub bvid: String,
    pub title: String,
    pub cover: Option<String>,
    pub duration: u32,
    pub upper: Upper,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upper {
    pub mid: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayUrlData {
    pub dash: Option<DashData>,
    pub durl: Option<Vec<DurlData>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashData {
    pub audio: Vec<AudioDash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDash {
    pub id: u32,
    pub base_url: Option<String>,
    pub backup_url: Option<Vec<String>>,
    pub bandwidth: u32,
    pub codecid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurlData {
    pub url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub bvid: String,
    pub aid: u64,
    pub cid: u64,
    pub title: String,
    #[serde(rename = "pic")]
    pub cover: Option<String>,
    pub duration: u32,
    pub owner: VideoOwner,
    pub pages: Vec<VideoPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoOwner {
    pub mid: u64,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoPage {
    pub cid: u64,
    pub page: u32,
    pub part: String,
    pub duration: u32,
}
