use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::{Duration,SystemTime, UNIX_EPOCH};

use anyhow::{bail, Context, Result};
use bytes::Bytes;
use parking_lot::RwLock;
use reqwest::{
    header::{HeaderMap, HeaderValue},
    StatusCode,
};
use url::form_urlencoded;
use serde::de::DeserializeOwned;
use quick_xml::de::from_str;
use quick_xml::Writer;
use quick_xml::se::Serializer as XmlSerializer;
use serde_json::json;
use serde::{Serialize,Deserialize};
use tracing::{debug, error, info, warn};
use httpdate;
use hmacsha::HmacSha;
use sha1::{Sha1, Digest};
use hex_literal::hex;
use base64::encode;




pub mod model;

pub use model::*;
pub use model::{PikpakFile, DateTime, FileType};

const ORIGIN: &str = "https://api-drive.mypikpak.com/drive/v1/files";
const REFERER: &str = "https://api-drive.mypikpak.com/drive/v1/files";
const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/92.0.4515.131 Safari/537.36";

#[derive(Debug, Clone)]
pub struct DriveConfig {
    pub api_base_url: String,
    pub refresh_token_url: String,
    pub workdir: Option<PathBuf>,
}




#[derive(Debug, Clone)]
pub struct PikpakDrive {
    config: DriveConfig,
    client: reqwest::blocking::Client,
    credentials: Arc<RwLock<Credentials>>,
    drive_id: Option<String>,
    pub nick_name: Option<String>,
}

impl PikpakDrive {
    pub fn new(config: DriveConfig, credentials:Credentials) -> Result<Self> {
        // let credentials = Credentials {
        //     refresh_token,
        //     access_token: None,
        // };
        debug!("credentials: {:?}", credentials);
        let mut headers = HeaderMap::new();
        headers.insert("Origin", HeaderValue::from_static(ORIGIN));
        headers.insert("Referer", HeaderValue::from_static(REFERER));
        let client = reqwest::blocking::Client::builder()
            .user_agent(UA)
            .default_headers(headers)
            // OSS closes idle connections after 60 seconds,
            // so we can close idle connections ahead of time to prevent re-using them.
            // See also https://github.com/hyperium/hyper/issues/2136
            .pool_idle_timeout(Duration::from_secs(50))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        let mut drive = Self {
            config,
            client,
            credentials: Arc::new(RwLock::new(credentials)),
            drive_id: None,
            nick_name: None,
        };

        let (tx, rx) = oneshot::channel();
        // schedule update token task
        let client = drive.clone();
        let refresh_token_from_file = if let Some(dir) = drive.config.workdir.as_ref() {
            fs::read_to_string(dir.join("refresh_token")).ok()
        } else {
            None
        };
        thread::spawn(move || {
            let mut delay_seconds = 7000;
            match client.do_refresh_token_with_retry(refresh_token_from_file) {
                Ok(res) => {
                    // token usually expires in 7200s, refresh earlier
                    delay_seconds = res.expires_in - 200;
                    if tx.send((res.access_token, res.token_type)).is_err() {
                        error!("send default drive id failed");
                    }
                }
                Err(err) => {
                    error!("refresh token failed: {}", err);
                    tx.send((String::new(), String::new())).unwrap();
                }
            }
            loop {
                thread::sleep(Duration::from_secs(delay_seconds));
                if let Err(err) = client.do_refresh_token_with_retry(None) {
                    error!("refresh token failed: {}", err);
                }
            }
        });

        let (drive_id, nick_name) = rx.recv()?;
        if drive_id.is_empty() {
            bail!("get default drive id failed");
        }
        //info!(drive_id = %drive_id, "found default drive");
        drive.drive_id = Some(drive_id);
        drive.nick_name = Some(nick_name);

        Ok(drive)
    }

    fn save_refresh_token(&self, refresh_token: &str) -> Result<()> {
        if let Some(dir) = self.config.workdir.as_ref() {
            fs::create_dir_all(dir)?;
            let refresh_token_file = dir.join("refresh_token");
            fs::write(refresh_token_file, refresh_token)?;
        }
        Ok(())
    }

    fn do_refresh_token(&self, user_name: &str,password: &str) -> Result<RefreshTokenResponse> {
        let mut data = HashMap::new();
        data.insert("captcha_token", "");
        data.insert("client_id", "YNxT9w7GMdWvEOKa");
        data.insert("client_secret", "dbw2OtmVEeuUvIptb1Coyg");
        data.insert("username", user_name);
        data.insert("password", password);

        let res = self
            .client
            .post(&self.config.refresh_token_url)
            .json(&data)
            .send()?;
        match res.error_for_status_ref() {
            Ok(_) => {
                let res = res.json::<RefreshTokenResponse>()?;
                info!(
                    refresh_token = %res.access_token,
                    "refresh token succeed"
                );
                Ok(res)
            }
            Err(err) => {
                let msg = res.text()?;
                let context = format!("{}: {}", err, msg);
                Err(err).context(context)
            }
        }
    }

    fn do_refresh_token_with_retry(
        &self,
        refresh_token_from_file: Option<String>,
    ) -> Result<RefreshTokenResponse> {
        let mut last_err = None;
        let mut refresh_token = self.refresh_token();

        let user_name = self.user_name();
        let password = self.password();
        for _ in 0..10 {
            match self.do_refresh_token(&user_name,&password) {
                Ok(res) => {
                    // let mut cred = self.credentials.write();
                    // cred.refresh_token = res.refresh_token.clone();
                    // cred.access_token = Some(res.access_token.clone());
                    // info!(
                    //     refresh_token = %res.access_token,
                    //     "get token succeed"
                    // );

                    if let Err(err) = self.save_refresh_token(&res.access_token) {
                        error!(error = %err, "save refresh token failed");
                    }
                    return Ok(res);
                }
                Err(err) => {
                    let mut should_warn = true;
                    let mut should_retry = match err.downcast_ref::<reqwest::Error>() {
                        Some(e) => {
                            e.is_connect()
                                || e.is_timeout()
                                || matches!(e.status(), Some(StatusCode::TOO_MANY_REQUESTS))
                        }
                        None => false,
                    };
                    // retry if command line refresh_token is invalid but we also have
                    // refresh_token from file
                    if let Some(refresh_token_from_file) = refresh_token_from_file.as_ref() {
                        if !should_retry && &refresh_token != refresh_token_from_file {
                            refresh_token = refresh_token_from_file.trim().to_string();
                            should_retry = true;
                            // don't warn if we are gonna try refresh_token from file
                            should_warn = false;
                        }
                    }
                    if should_retry {
                        if should_warn {
                            warn!(error = %err, "refresh token failed, will wait and retry");
                        }
                        last_err = Some(err);
                        thread::sleep(Duration::from_secs(1));
                        continue;
                    } else {
                        last_err = Some(err);
                        break;
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }


    fn user_name(&self) -> String {
        let cred = self.credentials.read();
        cred.username.clone()
    }

    fn password(&self) -> String {
        let cred = self.credentials.read();
        cred.password.clone()
    }

    fn refresh_token(&self) -> String {
        // let refresh_token_from_file = if let Some(dir) = self.config.workdir.as_ref() {
        //     fs::read_to_string(dir.join("refresh_token")).ok()
        // } else {
        //     None
        // };
        // refresh_token_from_file.unwrap().trim().to_string()
        "".to_string()
    }

    fn access_token(&self) -> Result<String> {
        let refresh_token_from_file = if let Some(dir) = self.config.workdir.as_ref() {
            fs::read_to_string(dir.join("refresh_token")).ok()
        } else {
            None
        };
        Ok(refresh_token_from_file.unwrap().trim().to_string())
    }

    fn drive_id(&self) -> Result<&str> {
        self.drive_id.as_deref().context("missing drive_id")
    }

    fn request<T, U>(&self, url: String, req: &T) -> Result<Option<U>>
    where
        T: Serialize + ?Sized,
        U: DeserializeOwned,
    {
        let mut access_token = self.access_token()?;
        let url = reqwest::Url::parse(&url)?;
        let res = self
            .client
            .get(url.clone())
            .bearer_auth(&access_token)
            .json(&req)
            .send()?
            .error_for_status();
        match res {
            Ok(res) => {
                if res.status() == StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                let res = res.json::<U>()?;
                Ok(Some(res))
            }
            Err(err) => {
                match err.status() {
                    Some(
                        status_code
                        @
                        // 4xx
                        (StatusCode::UNAUTHORIZED
                        | StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_MANY_REQUESTS
                        // 5xx
                        | StatusCode::INTERNAL_SERVER_ERROR
                        | StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT),
                    ) => {
                        if status_code == StatusCode::UNAUTHORIZED {
                            // refresh token and retry
                            let token_res = self.do_refresh_token_with_retry(None)?;
                            access_token = token_res.access_token;
                        } else {
                            // wait for a while and retry
                            thread::sleep(Duration::from_secs(1));
                        }
                        let res = self
                            .client
                            .post(url)
                            .bearer_auth(&access_token)
                            .json(&req)
                            .send()
                            ?
                            .error_for_status()?;
                        if res.status() == StatusCode::NO_CONTENT {
                            return Ok(None);
                        }
                        let res = res.json::<U>()?;
                        Ok(Some(res))
                    }
                    _ => Err(err.into()),
                }
            }
        }
    }


    fn post_request<T, U>(&self, url: String, req: &T) -> Result<Option<U>>
    where
        T: Serialize + ?Sized,
        U: DeserializeOwned,
    {
        let mut access_token = self.access_token()?;
        let url = reqwest::Url::parse(&url)?;
        let res = self
            .client
            .post(url.clone())
            .bearer_auth(&access_token)
            .json(&req)
            .send()?
            .error_for_status();
        match res {
            Ok(res) => {
                if res.status() == StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                let res = res.json::<U>()?;
                Ok(Some(res))
            }
            Err(err) => {
                match err.status() {
                    Some(
                        status_code
                        @
                        // 4xx
                        (StatusCode::UNAUTHORIZED
                        | StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_MANY_REQUESTS
                        // 5xx
                        | StatusCode::INTERNAL_SERVER_ERROR
                        | StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT),
                    ) => {
                        if status_code == StatusCode::UNAUTHORIZED {
                            // refresh token and retry
                            let token_res = self.do_refresh_token_with_retry(None)?;
                            access_token = token_res.access_token;
                        } else {
                            // wait for a while and retry
                            thread::sleep(Duration::from_secs(1));
                        }
                        let res = self
                            .client
                            .post(url)
                            .bearer_auth(&access_token)
                            .json(&req)
                            .send()
                            ?
                            .error_for_status()?;
                        if res.status() == StatusCode::NO_CONTENT {
                            return Ok(None);
                        }
                        let res = res.json::<U>()?;
                        Ok(Some(res))
                    }
                    _ => Err(err.into()),
                }
            }
        }
    }



    fn patch_request<T, U>(&self, url: String, req: &T) -> Result<Option<U>>
    where
        T: Serialize + ?Sized,
        U: DeserializeOwned,
    {
        let mut access_token = self.access_token()?;
        let url = reqwest::Url::parse(&url)?;
        let res = self
            .client
            .patch(url.clone())
            .bearer_auth(&access_token)
            .json(&req)
            .send()?
            .error_for_status();
        match res {
            Ok(res) => {
                if res.status() == StatusCode::NO_CONTENT {
                    return Ok(None);
                }
                let res = res.json::<U>()?;
                Ok(Some(res))
            }
            Err(err) => {
                match err.status() {
                    Some(
                        status_code
                        @
                        // 4xx
                        (StatusCode::UNAUTHORIZED
                        | StatusCode::REQUEST_TIMEOUT
                        | StatusCode::TOO_MANY_REQUESTS
                        // 5xx
                        | StatusCode::INTERNAL_SERVER_ERROR
                        | StatusCode::BAD_GATEWAY
                        | StatusCode::SERVICE_UNAVAILABLE
                        | StatusCode::GATEWAY_TIMEOUT),
                    ) => {
                        if status_code == StatusCode::UNAUTHORIZED {
                            // refresh token and retry
                            let token_res = self.do_refresh_token_with_retry(None)?;
                            access_token = token_res.access_token;
                        } else {
                            // wait for a while and retry
                            thread::sleep(Duration::from_secs(1));
                        }
                        let res = self
                            .client
                            .post(url)
                            .bearer_auth(&access_token)
                            .json(&req)
                            .send()
                            ?
                            .error_for_status()?;
                        if res.status() == StatusCode::NO_CONTENT {
                            return Ok(None);
                        }
                        let res = res.json::<U>()?;
                        Ok(Some(res))
                    }
                    _ => Err(err.into()),
                }
            }
        }
    }






    pub fn list_all(&self, parent_file_id: &str) -> Result<Vec<PikpakFile>> {
        info!("drive list_all: {}", parent_file_id);
        let mut files = Vec::new();
        let mut marker = None;
        loop {
            let res = self.list(parent_file_id, marker.as_deref())?;
            //println!("drive list  result is:{:?}",res);
            files.extend(res.files.into_iter());
            if res.next_page_token.is_empty() {
                break;
            }
            marker = Some(res.next_page_token);
        }
        Ok(files)
    }

    pub fn list(&self, parent_file_id: &str, marker: Option<&str>) -> Result<ListFileResponse> {
        let drive_id = self.drive_id()?;
        let pagetoken = marker.unwrap_or("");
        let mut data = HashMap::new();
        data.insert("parent_id", parent_file_id);
        let mut rurl = format!("{}?parent_id={}&thumbnail_size=SIZE_LARGE&with_audit=true&page_token={}&limit=0&filters={{\"phase\":{{\"eq\":\"PHASE_TYPE_COMPLETE\"}},\"trashed\":{{\"eq\":false}}}}",self.config.api_base_url,&parent_file_id,pagetoken);
        self.request(rurl, &data).and_then(|res| res.context("expect response"))
    }


    pub fn create_folder(&self, parent_id:&str, folder_name: &str) -> Result<CreateFolderResponse> {
        debug!("drive create folder {}", folder_name);
        let rurl = format!("{}",self.config.api_base_url);
        let req = CreateFolderRequest{kind:"drive#folder",name:folder_name,parent_id:parent_id};
        self.post_request(rurl, &req).and_then(|res| res.context("expect response"))
    }


    pub fn remove_file(&self, file_id: &str) -> Result<TaskResponse> {
        debug!("drive remove file {}", file_id);
        let rurl = format!("{}:batchDelete",self.config.api_base_url);
        let req = DelFileRequest{ids:vec![file_id.to_string()]};
        self.post_request(rurl,&req).and_then(|res| res.context("expect response"))
    }


    pub fn move_file(&self, file_id: &str, new_parent_id: &str) -> Result<TaskResponse>  {
        let rurl = format!("{}:batchMove",self.config.api_base_url);
        let req = MoveFileRequest{ids:vec![file_id.to_string()],to:MoveTo { parent_id: new_parent_id.to_string()}};
        self.post_request(rurl,&req).and_then(|res| res.context("expect response"))
    }


    pub fn rename_file(&self, file_id: &str, new_name: &str) -> Result<PikpakFile> {
        let rurl = format!("{}/{}",self.config.api_base_url,file_id);
        let req = RenameFileRequest{name:new_name};
        self.patch_request(rurl,&req).and_then(|res| res.context("expect response"))
    }



    pub fn copy_file(&self, file_id: &str, new_parent_id: &str) -> Result<TaskResponse> {
        let rurl = format!("{}:batchCopy",self.config.api_base_url);
        let req = MoveFileRequest{ids:vec![file_id.to_string()],to:MoveTo { parent_id: new_parent_id.to_string()}};
        self.post_request(rurl,&req).and_then(|res| res.context("expect response"))
    }

    pub fn create_file_with_proof(&self,name: &str, parent_file_id: &str, hash:&str, size: u64) ->  Result<UploadResponse> {
        info!("drive create file with proof {}", name);
        let url = format!("{}",self.config.api_base_url);
        let req = UploadRequest{
            kind:"drive#file".to_string(),
		    name:name.to_string(),
		    size:size,
		    hash: hash.to_string(),
		    upload_type: "UPLOAD_TYPE_RESUMABLE".to_string(),
            objProvider: ObjProvider { provider: "UPLOAD_TYPE_UNKNOWN".to_string() },
		    parent_id:parent_file_id.to_string(),
        };
        let payload = serde_json::to_string(&req).unwrap();
        let access_token_key = "access_token".to_string();
        let access_token = self.access_token().unwrap();

        let res = self.client.post(url)
            .header(reqwest::header::CONTENT_LENGTH, payload.len())
            .header(reqwest::header::HOST, "api-drive.mypikpak.com")
            .header(reqwest::header::AUTHORIZATION, format!("Bearer {}",access_token))
            .body(payload)
            .send();

        let body = match res {
            Ok(res) => res.text().unwrap(),
            Err(err) => {
                error!("{:?}", err);
                return Err(err.into());
            }
        };
            
        let result = match serde_json::from_str::<UploadResponse>(&body) {
            Ok(result) => result,
            Err(e) => {
                error!(error = %e, "create_file_with_proof");
                return Err(e.into());
            }
        };
    
        Ok(result)
    }


    pub fn get_pre_upload_info(&self,oss_args:&OssArgs) -> Result<String> {
        let mut url = format!("https://{}/{}?uploads",oss_args.endpoint,oss_args.key);
        let now = SystemTime::now();
        let gmt = httpdate::fmt_http_date(now);
        let mut req = self.client.post(url)
            .header(reqwest::header::USER_AGENT, "aliyun-sdk-android/2.9.5(Linux/Android 11/ONEPLUS%20A6000;RKQ1.201217.002)")
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Oss-Security-Token", &oss_args.security_token)
            .header("Date", &gmt).build()?;
        let oss_sign:String = self.hmac_authorization(&req,&gmt,oss_args);
        let oss_header = format!("OSS {}:{}",&oss_args.access_key_id,&oss_sign);
        let header_auth = HeaderValue::from_str(&oss_header).unwrap();
        req.headers_mut().insert(reqwest::header::AUTHORIZATION, header_auth);
        let res = self.client.execute(req);
        let body = match res {
            Ok(res) => res.text().unwrap(),
            Err(err) => {
                error!("{:?}", err);
                return Err(err.into());
            }
        };
        let result: InitiateMultipartUploadResult = from_str(&body).unwrap();
        Ok(result.UploadId.clone())
    }

    pub fn upload_chunk(&self, file:&PikpakFile, oss_args:&OssArgs, upload_id:&str, current_chunk:u64,body: Bytes) -> Result<(PartInfo)> {
        debug!(file_name=%file.name,upload_id = upload_id,current_chunk=current_chunk, "upload_chunk");
        let encoded: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("partNumber", current_chunk.to_string().as_str())
        .append_pair("uploadId", upload_id)
        .finish();

        let url = format!("https://{}/{}?{}",oss_args.endpoint,oss_args.key,encoded);
   
        let now = SystemTime::now();
        let gmt = httpdate::fmt_http_date(now);
        let mut req = self.client.put(url)
            .body(body)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Oss-Security-Token", &oss_args.security_token)
            .header("Date", &gmt).build()?;
        let oss_sign:String = self.hmac_authorization(&req,&gmt,oss_args);
        let oss_header = format!("OSS {}:{}",&oss_args.access_key_id,&oss_sign);
        let header_auth = HeaderValue::from_str(&oss_header).unwrap();
        req.headers_mut().insert(reqwest::header::AUTHORIZATION, header_auth);
        let res = self.client.execute(req);
        // let body = match res {
        //     Ok(res) => res.text().unwrap(),
        //     Err(err) => {
        //         error!("{:?}", err);
        //         return Err(err.into());
        //     }
        // };
        //let body = &res.text().await?;

        let etag  = match &res.unwrap().headers().get("ETag") {
            Some(etag) => etag.to_str().unwrap().to_string(),
            None => "".to_string(),
        };
            
        let part = PartInfo {
            PartNumber: PartNumber { PartNumber: current_chunk },
            ETag: etag,
        };
        
        Ok(part)
    }

    pub fn complete_upload(&self,file:&PikpakFile, upload_tags:String, oss_args:&OssArgs, upload_id:&str)-> Result<()> {
        info!(file = %file.name, "complete_upload");
        let url = format!("https://{}/{}?uploadId={}",oss_args.endpoint,oss_args.key,upload_id);
        let now = SystemTime::now();
        let gmt = httpdate::fmt_http_date(now);
        let mut req = self.client.post(url)
            .body(upload_tags)
            .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
            .header("X-Oss-Security-Token", &oss_args.security_token)
            .header("Date", &gmt).build()?;
        let oss_sign:String = self.hmac_authorization(&req,&gmt,oss_args);
        let oss_header = format!("OSS {}:{}",&oss_args.access_key_id,&oss_sign);
        let header_auth = HeaderValue::from_str(&oss_header).unwrap();
        req.headers_mut().insert(reqwest::header::AUTHORIZATION, header_auth);
        let res = self.client.execute(req);

        let body = match res {
            Ok(res) => res.text().unwrap(),
            Err(err) => {
                error!("{:?}", err);
                return Err(err.into());
            }
        };
        info!(file = %file.name, res_body = body, "complete_upload_response");
        Ok(())
    }

    pub fn hmac_authorization(&self, req:&reqwest::blocking::Request,time:&str,oss_args:&OssArgs)->String{
        let message = format!("{}\n\n{}\n{}\nx-oss-security-token:{}\n/{}{}?{}",req.method().as_str(),req.headers().get(reqwest::header::CONTENT_TYPE).unwrap().to_str().unwrap(),time,oss_args.security_token,oss_args.bucket,req.url().path(),req.url().query().unwrap());
        let key = &oss_args.access_key_secret;
      
        let mut hasher = HmacSha::from(key, &message, Sha1::default());
        let result = hasher.compute_digest();
        let signature_base64 = base64::encode(&result);
        signature_base64
    }



    pub fn download(&self, url: &str, start_pos: u64, size: usize) -> Result<Bytes> {
        use reqwest::header::RANGE;

        let end_pos = start_pos + size as u64 - 1;
        debug!(url = %url, start = start_pos, end = end_pos, "download file");
        let range = format!("bytes={}-{}", start_pos, end_pos);
        let res = self
            .client
            .get(url)
            .header(RANGE, range)
            .send()?
            .error_for_status()?;
        Ok(res.bytes()?)
    }

    pub fn get_download_url(&self, file_id: &str) -> Result<String> {
        debug!(file_id = %file_id, "get download url");
        // let req = GetFileDownloadUrlRequest {
        //     drive_id: self.drive_id()?,
        //     file_id,
        // };
        // let res: GetFileDownloadUrlResponse = self
        //     .request(
        //         format!("{}/v2/file/get_download_url", self.config.api_base_url),
        //         &req,
        //     )?
        //     .context("expect response")?;

        let mut rurl = format!("{}/{}",self.config.api_base_url,file_id.to_string());
        let url = rurl;
        let mut data = HashMap::new();
        data.insert("file_id", file_id);
        let res: PikpakFile = self.request(url,&data)?.context("expect response")?;
        
        if res.mime_type.contains("video/"){
            Ok(res.medias[0].link.url.clone())
        }else{
            Ok(res.web_content_link.clone())
        }
    }

    pub fn get_quota(&self) -> Result<(u64, u64)> {
        let drive_id = self.drive_id()?;
        let mut data = HashMap::new();
        data.insert("drive_id", drive_id);
        let res: GetDriveResponse = self
            .request(format!("{}/v2/drive/get", self.config.api_base_url), &data)?
            .context("expect response")?;
        Ok((res.used_size, res.total_size))
    }
}
