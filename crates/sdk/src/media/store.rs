//! Private object storage. Local files for development, R2 for a public host.
//!
//! Neither implementation publishes an object URL. A read is a stream or a
//! five-minute presigned GET that Ecclesia minted after checking the viewer.

use std::path::PathBuf;
use std::time::Duration;

use rusty_s3::{Bucket, Credentials, S3Action, UrlStyle};

use crate::host::R2Config;

use super::{MediaError, PRESIGN_SECONDS, WEBP_CONTENT_TYPE};

/// How the response may be cached after an authorized read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaCaching {
    /// Staged bytes. Do not store the response.
    NoStore,
    /// Attached bytes. A private cache may keep them for `seconds`.
    Private { seconds: u64 },
}

/// Bytes or a short-lived object URL. The URL is never a public bucket address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaDelivery {
    Stream {
        bytes: Vec<u8>,
        content_type: &'static str,
        caching: MediaCaching,
    },
    Presigned {
        url: String,
        expires_in_secs: u64,
        content_type: &'static str,
        caching: MediaCaching,
    },
}

/// Puts, copies, and deletes normalized WebP objects.
///
/// # Notes
/// `deliver` is the only read used after authorization. Local stores return
/// bytes. R2 returns a presigned URL and does not fetch the object.
pub trait ObjectStore: Send + Sync {
    fn put<'a>(
        &'a self,
        key: &'a str,
        bytes: &'a [u8],
    ) -> impl std::future::Future<Output = Result<(), MediaError>> + Send + 'a;

    fn get<'a>(
        &'a self,
        key: &'a str,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, MediaError>> + Send + 'a;

    fn copy<'a>(
        &'a self,
        from: &'a str,
        to: &'a str,
    ) -> impl std::future::Future<Output = Result<(), MediaError>> + Send + 'a;

    fn delete<'a>(
        &'a self,
        key: &'a str,
    ) -> impl std::future::Future<Output = Result<(), MediaError>> + Send + 'a;

    fn deliver<'a>(
        &'a self,
        key: &'a str,
        caching: MediaCaching,
    ) -> impl std::future::Future<Output = Result<MediaDelivery, MediaError>> + Send + 'a;
}

/// Files under one directory. The directory is created on the first put.
#[derive(Debug, Clone)]
pub struct LocalStore {
    root: PathBuf,
}

impl LocalStore {
    /// # Parameters
    /// - `root`: private directory. It is not the public static tree.
    pub fn open(root: PathBuf) -> Self {
        Self { root }
    }

    fn path(&self, key: &str) -> Result<PathBuf, MediaError> {
        if !key_is_relative(key) {
            return Err(MediaError::Store);
        }
        Ok(self.root.join(key))
    }
}

impl ObjectStore for LocalStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), MediaError> {
        let path = self.path(key)?;
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|_| MediaError::Store)?;
        }
        tokio::fs::write(path, bytes)
            .await
            .map_err(|_| MediaError::Store)
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        let path = self.path(key)?;
        match tokio::fs::read(&path).await {
            Ok(bytes) => Ok(bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(MediaError::NotFound),
            Err(_) => Err(MediaError::Store),
        }
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), MediaError> {
        let source = self.path(from)?;
        let dest = self.path(to)?;
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|_| MediaError::Store)?;
        }
        match tokio::fs::copy(source, dest).await {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(MediaError::Store),
            Err(_) => Err(MediaError::Store),
        }
    }

    async fn delete(&self, key: &str) -> Result<(), MediaError> {
        let path = self.path(key)?;
        match tokio::fs::remove_file(path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(_) => Err(MediaError::Store),
        }
    }

    async fn deliver(&self, key: &str, caching: MediaCaching) -> Result<MediaDelivery, MediaError> {
        let bytes = self.get(key).await?;
        Ok(MediaDelivery::Stream {
            bytes,
            content_type: WEBP_CONTENT_TYPE,
            caching,
        })
    }
}

/// Private Cloudflare R2 through the S3 API. Object URLs are presigned.
pub struct R2Store {
    bucket: Bucket,
    credentials: Credentials,
    client: reqwest::Client,
}

impl R2Store {
    /// Opens the account endpoint `https://{account}.r2.cloudflarestorage.com`.
    ///
    /// # Errors
    /// [`MediaError::Store`] when the account id or bucket cannot form that endpoint.
    pub fn open(config: R2Config) -> Result<Self, MediaError> {
        let endpoint = format!(
            "https://{}.r2.cloudflarestorage.com",
            config.account_id.trim()
        );
        Self::with_endpoint(config, &endpoint)
    }

    pub(crate) fn with_endpoint(config: R2Config, endpoint: &str) -> Result<Self, MediaError> {
        let endpoint = url::Url::parse(endpoint).map_err(|_| MediaError::Store)?;
        let bucket = Bucket::new(endpoint, UrlStyle::Path, config.bucket, "auto")
            .map_err(|_| MediaError::Store)?;
        let credentials = Credentials::new(config.access_key_id, config.secret_access_key);
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| MediaError::Store)?;
        Ok(Self {
            bucket,
            credentials,
            client,
        })
    }

    fn signed_get(&self, key: &str) -> Result<String, MediaError> {
        let action = self.bucket.get_object(Some(&self.credentials), key);
        let url = action
            .sign(Duration::from_secs(PRESIGN_SECONDS))
            .to_string();
        if url.contains("r2.dev") {
            return Err(MediaError::Store);
        }
        Ok(url)
    }

    fn signed_put(&self, key: &str) -> Result<url::Url, MediaError> {
        let mut action = self.bucket.put_object(Some(&self.credentials), key);
        action
            .headers_mut()
            .insert("content-type", WEBP_CONTENT_TYPE);
        Ok(action.sign(Duration::from_secs(120)))
    }

    fn signed_delete(&self, key: &str) -> url::Url {
        self.bucket
            .delete_object(Some(&self.credentials), key)
            .sign(Duration::from_secs(120))
    }
}

impl ObjectStore for R2Store {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), MediaError> {
        let url = self.signed_put(key)?;
        let response = self
            .client
            .put(url)
            .header(reqwest::header::CONTENT_TYPE, WEBP_CONTENT_TYPE)
            .body(bytes.to_vec())
            .send()
            .await
            .map_err(|_| MediaError::Store)?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(MediaError::Store)
        }
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        let action = self.bucket.get_object(Some(&self.credentials), key);
        let url = action.sign(Duration::from_secs(120));
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| MediaError::Store)?;
        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Err(MediaError::NotFound);
        }
        if !response.status().is_success() {
            return Err(MediaError::Store);
        }
        let bytes = response.bytes().await.map_err(|_| MediaError::Store)?;
        Ok(bytes.to_vec())
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), MediaError> {
        let bytes = self.get(from).await.map_err(|error| match error {
            MediaError::NotFound => MediaError::Store,
            other => other,
        })?;
        self.put(to, &bytes).await
    }

    async fn delete(&self, key: &str) -> Result<(), MediaError> {
        let url = self.signed_delete(key);
        let response = self
            .client
            .delete(url)
            .send()
            .await
            .map_err(|_| MediaError::Store)?;
        if response.status().is_success() || response.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(MediaError::Store)
        }
    }

    async fn deliver(&self, key: &str, caching: MediaCaching) -> Result<MediaDelivery, MediaError> {
        let url = self.signed_get(key)?;
        Ok(MediaDelivery::Presigned {
            url,
            expires_in_secs: PRESIGN_SECONDS,
            content_type: WEBP_CONTENT_TYPE,
            caching,
        })
    }
}

/// The store selected for this process. Public hosts use R2. Local hosts use disk.
pub enum ConfiguredStore {
    Local(LocalStore),
    R2(R2Store),
}

impl ObjectStore for ConfiguredStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), MediaError> {
        match self {
            Self::Local(store) => store.put(key, bytes).await,
            Self::R2(store) => store.put(key, bytes).await,
        }
    }

    async fn get(&self, key: &str) -> Result<Vec<u8>, MediaError> {
        match self {
            Self::Local(store) => store.get(key).await,
            Self::R2(store) => store.get(key).await,
        }
    }

    async fn copy(&self, from: &str, to: &str) -> Result<(), MediaError> {
        match self {
            Self::Local(store) => store.copy(from, to).await,
            Self::R2(store) => store.copy(from, to).await,
        }
    }

    async fn delete(&self, key: &str) -> Result<(), MediaError> {
        match self {
            Self::Local(store) => store.delete(key).await,
            Self::R2(store) => store.delete(key).await,
        }
    }

    async fn deliver(&self, key: &str, caching: MediaCaching) -> Result<MediaDelivery, MediaError> {
        match self {
            Self::Local(store) => store.deliver(key, caching).await,
            Self::R2(store) => store.deliver(key, caching).await,
        }
    }
}

fn key_is_relative(key: &str) -> bool {
    if key.is_empty() || key.starts_with('/') || key.contains('\\') {
        return false;
    }
    key.split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Selects the process store.
///
/// # Returns
/// A local directory, or R2 when [`crate::host::load_media_host`] has the four
/// public-host variables.
///
/// # Errors
/// [`MediaError::StorageUnavailable`] when a public host has no R2 configuration.
/// Callers map that on the media route. Process boot does not call this.
pub fn open_configured_store() -> Result<ConfiguredStore, MediaError> {
    match crate::host::load_media_host().map_err(|_| MediaError::StorageUnavailable)? {
        crate::host::MediaEnv::NotRequired => Ok(ConfiguredStore::Local(LocalStore::open(
            crate::host::local_media_directory_from_env(),
        ))),
        crate::host::MediaEnv::R2(config) => Ok(ConfiguredStore::R2(R2Store::open(config)?)),
    }
}
