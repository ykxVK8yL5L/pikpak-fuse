use std::ops;
use std::time::SystemTime;

use ::time::{format_description::well_known::Rfc3339, OffsetDateTime};
use serde::{Deserialize, Deserializer, Serialize};



#[derive(Debug, Clone)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}



#[derive(Debug, Clone, Deserialize)]
pub struct RefreshTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
}


#[derive(Debug, Clone, Serialize)]
pub struct ListFileRequest<'a> {
    pub drive_id: &'a str,
    pub parent_file_id: &'a str,
    pub limit: u64,
    pub all: bool,
    pub image_thumbnail_process: &'a str,
    pub image_url_process: &'a str,
    pub video_thumbnail_process: &'a str,
    pub fields: &'a str,
    pub order_by: &'a str,
    pub order_direction: &'a str,
    pub marker: Option<&'a str>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListFileResponse {
    pub kind : String,
    pub next_page_token : String,
    pub files: Vec<PikpakFile>,
}




#[derive(Debug, Clone, Serialize)]
pub struct GetFileDownloadUrlRequest<'a> {
    pub drive_id: &'a str,
    pub file_id: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetFileDownloadUrlResponse {
    pub url: String,
    pub size: u64,
    pub expiration: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetDriveResponse {
    pub total_size: u64,
    pub used_size: u64,
}


#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Folder,
    File,
}


#[derive(Debug, Clone,Serialize)]
pub struct DateTime(SystemTime);

impl DateTime {
    pub fn new(st: SystemTime) -> Self {
        Self(st)
    }
}

impl<'a> Deserialize<'a> for DateTime {
    fn deserialize<D: Deserializer<'a>>(deserializer: D) -> Result<Self, D::Error> {
        let dt = OffsetDateTime::parse(<&str>::deserialize(deserializer)?, &Rfc3339)
            .map_err(serde::de::Error::custom)?;
        Ok(Self(dt.into()))
    }
}

impl ops::Deref for DateTime {
    type Target = SystemTime;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct PikpakFile {
    pub kind: String,
    pub id: String,
    pub parent_id: String,
    pub name: String,
    pub size: String,
    pub file_extension: String,
    pub mime_type: String,
    pub web_content_link: String,
    pub created_time: DateTime,
    pub modified_time: DateTime,
    pub medias:Vec<Media>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Link {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub media_name: String,
    pub link:Link,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesList {
    pub kind : String,
    pub next_page_token : String,
    pub files: Vec<PikpakFile>,
}


impl PikpakFile {
    pub fn new_root() -> Self {
        let now = SystemTime::now();
        Self {
            kind: "drive#folder".to_string(),
            id: "".to_string(),
            parent_id: "".to_string(),
            name: "root".to_string(),
            size: "0".to_string(),
            created_time: DateTime(now),
            modified_time: DateTime(now),
            file_extension: "".to_string(),
            mime_type: "".to_string(),
            web_content_link: "".to_string(),
            medias:Vec::new(),
        }
    }
}
