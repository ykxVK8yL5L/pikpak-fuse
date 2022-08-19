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
    pub phase: String,
    pub name: String,
    pub size: String,
    pub file_extension: String,
    pub mime_type: String,
    pub web_content_link: String,
    pub created_time: DateTime,
    pub modified_time: DateTime,
    pub medias:Vec<Media>,
    pub hash: Option<String>,
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


#[derive(Debug, Clone, Serialize)]
pub struct CreateFolderRequest<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub parent_id: &'a str,
}


#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct CreateFolderResponse{
    pub upload_type: String,
    pub file: PikpakFile,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct TaskResponse{
    pub task_id: String,
}



#[derive(Debug, Clone, Serialize)]
pub struct DelFileRequest {
    pub ids: Vec<String>,
}


#[derive(Debug, Clone, Serialize)]
pub struct MoveFileRequest {
    pub ids: Vec<String>,
    pub to: MoveTo,
}


#[derive(Debug, Clone, Serialize)]
pub struct MoveTo {
    pub parent_id: String,
}


#[derive(Debug, Clone, Serialize)]
pub struct RenameFileRequest<'a>{
    pub name: &'a str,
}


#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct UploadRequest {
    pub kind: String,
    pub name: String,
    pub size: u64,
    pub hash: String,
    pub upload_type: String,
    pub objProvider:ObjProvider,
    pub parent_id: String,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct ObjProvider {
    pub provider: String,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct OssArgs {
    pub bucket: String,
    pub endpoint: String,
    pub access_key_id: String,
    pub access_key_secret: String,
    pub key: String,
    pub security_token: String,
}


#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct CompleteMultipartUpload {
    pub Part: Vec<PartInfo>,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct PartInfo {
    #[serde(flatten)]
    pub PartNumber: PartNumber,
    pub ETag: String,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct PartNumber {
    pub PartNumber: u64,
}




#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct UploadResponse {
    pub upload_type: String,
    pub resumable: Resumable,
    pub file: PikpakFile,
}


#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct Resumable {
    pub kind: String,
    pub provider: String,
    pub params: UploadParams,
}

#[derive(Debug, Clone,Serialize, Deserialize)]
pub struct UploadParams {
    pub access_key_id: String,
    pub access_key_secret: String,
    pub bucket: String,
    pub endpoint: String,
    pub expiration: String,
    pub key: String,
    pub security_token: String,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct InitiateMultipartUploadResult {
    pub Bucket: String,
    pub Key: String,
    pub UploadId: String,
}





impl PikpakFile {
    pub fn new_root() -> Self {
        let now = SystemTime::now();
        Self {
            kind: "drive#folder".to_string(),
            id: "".to_string(),
            parent_id: "".to_string(),
            phase: "".to_string(),
            name: "root".to_string(),
            size: "0".to_string(),
            created_time: DateTime(now),
            modified_time: DateTime(now),
            file_extension: "".to_string(),
            mime_type: "".to_string(),
            web_content_link: "".to_string(),
            medias:Vec::new(),
            hash: Some("".to_string()),
        }
    }
}
