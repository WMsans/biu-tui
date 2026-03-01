use anyhow::Result;
use crate::api::{BilibiliClient, ApiResponse, FavoriteFolder, FavoriteResource};

#[derive(Debug, Clone, serde::Deserialize)]
struct FavoriteListData {
    list: Option<Vec<FavoriteFolder>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct FavoriteResourceData {
    medias: Option<Vec<FavoriteResource>>,
    has_more: bool,
}

impl BilibiliClient {
    pub async fn get_created_folders(&self, mid: u64) -> Result<Vec<FavoriteFolder>> {
        let path = format!("/x/v3/fav/folder/created/list-all?up_mid={}", mid);
        let response: ApiResponse<FavoriteListData> = self.get(&path).await
            .map_err(|e| anyhow::anyhow!("Failed to get created folders: {}", e))?;

        Ok(response.data.and_then(|d| d.list).unwrap_or_default())
    }

    pub async fn get_collected_folders(&self, mid: u64) -> Result<Vec<FavoriteFolder>> {
        let path = format!("/x/v3/fav/folder/collected/list?up_mid={}&ps=20", mid);
        let response: ApiResponse<FavoriteListData> = self.get(&path).await
            .map_err(|e| anyhow::anyhow!("Failed to get collected folders: {}", e))?;

        Ok(response.data.and_then(|d| d.list).unwrap_or_default())
    }

    pub async fn get_folder_resources(&self, folder_id: u64, page: u32) -> Result<(Vec<FavoriteResource>, bool)> {
        let path = format!(
            "/x/v3/fav/resource/list?media_id={}&ps=20&pn={}",
            folder_id, page
        );
        let response: ApiResponse<FavoriteResourceData> = self.get(&path).await
            .map_err(|e| anyhow::anyhow!("Failed to get folder resources: {}", e))?;

        let data = response.data.unwrap_or(FavoriteResourceData {
            medias: None,
            has_more: false,
        });

        Ok((data.medias.unwrap_or_default(), data.has_more))
    }
}
