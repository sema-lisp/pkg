use sha2::{Digest, Sha256};
use std::io;
use std::path::Path;

use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, PutPayload};

use crate::config::Config;

/// Where package tarballs live. Blobs are content-addressed (`<sha256>.tar.gz`)
/// and sharded by the first two hex chars (`ab/abcd….tar.gz`) — the same layout
/// on both backends, so data migrates one-for-one.
///
/// Filesystem by default; S3-compatible object storage (e.g. Cloudflare R2) when
/// `BLOB_S3_BUCKET` is configured — which decouples tarball durability from the
/// compute node (required for stateless / multi-instance deploys).
pub enum BlobStore {
    Fs { dir: String },
    S3 { store: object_store::aws::AmazonS3 },
}

pub fn content_key(data: &[u8]) -> (String, String, usize) {
    let hex = format!("{:x}", Sha256::digest(data));
    (format!("{hex}.tar.gz"), hex, data.len())
}

/// `ab/abcd….tar.gz` — shard by the first two chars, identical on fs and S3.
fn shard(key: &str) -> String {
    format!("{}/{}", &key[..2], key)
}

impl BlobStore {
    /// Build from config: S3 when `BLOB_S3_BUCKET` is set, else local filesystem.
    pub fn from_config(config: &Config) -> Result<Self, String> {
        let Some(bucket) = config.blob_s3_bucket.clone() else {
            return Ok(BlobStore::Fs {
                dir: config.blob_dir.clone(),
            });
        };
        let access = config
            .blob_s3_access_key_id
            .clone()
            .ok_or("BLOB_S3_ACCESS_KEY_ID is required when BLOB_S3_BUCKET is set")?;
        let secret = config
            .blob_s3_secret_access_key
            .clone()
            .ok_or("BLOB_S3_SECRET_ACCESS_KEY is required when BLOB_S3_BUCKET is set")?;
        let mut builder = AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_access_key_id(access)
            .with_secret_access_key(secret)
            // R2 has no regions; "auto" is the conventional value it accepts.
            .with_region(
                config
                    .blob_s3_region
                    .clone()
                    .unwrap_or_else(|| "auto".into()),
            );
        if let Some(endpoint) = &config.blob_s3_endpoint {
            builder = builder
                .with_endpoint(endpoint.clone())
                .with_allow_http(endpoint.starts_with("http://"));
        }
        let store = builder
            .build()
            .map_err(|e| format!("failed to init S3 blob store: {e}"))?;
        Ok(BlobStore::S3 { store })
    }

    /// A short human label for logs.
    pub fn describe(&self) -> String {
        match self {
            BlobStore::Fs { dir } => format!("filesystem ({dir})"),
            BlobStore::S3 { .. } => "S3-compatible object storage".into(),
        }
    }

    /// Store bytes; returns (blob_key, sha256_hex, size). Content-addressed, so a
    /// failure downstream leaves at worst one orphan a retried publish reuses.
    pub async fn store(&self, data: &[u8]) -> io::Result<(String, String, usize)> {
        let (key, hex, size) = content_key(data);
        match self {
            BlobStore::Fs { dir } => {
                let path = Path::new(dir).join(&key[..2]).join(&key);
                if let Some(parent) = path.parent() {
                    tokio::fs::create_dir_all(parent).await?;
                }
                tokio::fs::write(&path, data).await?;
            }
            BlobStore::S3 { store } => {
                let path = ObjectPath::from(shard(&key));
                store
                    .put(&path, PutPayload::from(data.to_vec()))
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?;
            }
        }
        Ok((key, hex, size))
    }

    /// Read blob bytes, or `None` if the object is missing.
    pub async fn read(&self, key: &str) -> Option<Vec<u8>> {
        match self {
            BlobStore::Fs { dir } => tokio::fs::read(Path::new(dir).join(&key[..2]).join(key))
                .await
                .ok(),
            BlobStore::S3 { store } => {
                let path = ObjectPath::from(shard(key));
                let result = store.get(&path).await.ok()?;
                result.bytes().await.ok().map(|b| b.to_vec())
            }
        }
    }

    /// Delete a blob. A missing object is treated as success (idempotent), since
    /// content-addressed blobs may already have been reclaimed.
    pub async fn delete(&self, key: &str) -> io::Result<()> {
        match self {
            BlobStore::Fs { dir } => {
                let path = Path::new(dir).join(&key[..2]).join(key);
                match tokio::fs::remove_file(&path).await {
                    Ok(()) => Ok(()),
                    Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
                    Err(e) => Err(e),
                }
            }
            BlobStore::S3 { store } => {
                let path = ObjectPath::from(shard(key));
                match store.delete(&path).await {
                    Ok(()) => Ok(()),
                    Err(object_store::Error::NotFound { .. }) => Ok(()),
                    Err(e) => Err(io::Error::other(e.to_string())),
                }
            }
        }
    }
}
