// pkg/tests/blob_test.rs

use std::env;

use sema_pkg::blob::{content_key, BlobStore};
use sema_pkg::config::Config;

#[tokio::test]
async fn test_content_key() {
    let data = b"test data";
    let (key, hex, size) = content_key(data);
    assert_eq!(
        key,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9.tar.gz"
    );
    assert_eq!(
        hex,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
    );
    assert_eq!(size, 9);
}

#[tokio::test]
async fn test_filesystem_blob_store() {
    let temp_dir = tempfile::tempdir().unwrap();
    let blob_dir = temp_dir.path().to_str().unwrap().to_string();
    let config = Config {
        blob_dir: blob_dir.clone(),
        ..Default::default()
    };
    let blob_store = BlobStore::from_config(&config).unwrap();

    let data = b"test data";
    let (key, hex, size) = blob_store.store(data).await.unwrap();
    assert_eq!(
        key,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9.tar.gz"
    );
    assert_eq!(
        hex,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
    );
    assert_eq!(size, 9);

    let retrieved_data = blob_store.read(&key).await.unwrap();
    assert_eq!(retrieved_data, data);
}

#[tokio::test]
async fn test_s3_blob_store() {
    // Skip this test if S3 credentials are not available
    if env::var("BLOB_S3_BUCKET").is_err() {
        return;
    }

    let config = Config {
        blob_dir: "data/blobs".to_string(),
        blob_s3_bucket: env::var("BLOB_S3_BUCKET").ok(),
        blob_s3_endpoint: env::var("BLOB_S3_ENDPOINT").ok(),
        blob_s3_region: env::var("BLOB_S3_REGION").ok(),
        blob_s3_access_key_id: env::var("BLOB_S3_ACCESS_KEY_ID").ok(),
        blob_s3_secret_access_key: env::var("BLOB_S3_SECRET_ACCESS_KEY").ok(),
        ..Default::default()
    };
    let blob_store = BlobStore::from_config(&config).unwrap();

    let data = b"test data";
    let (key, hex, size) = blob_store.store(data).await.unwrap();
    assert_eq!(
        key,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9.tar.gz"
    );
    assert_eq!(
        hex,
        "916f0027a575074ce72a331777c3478d6513f786a591bd892da1a577bf2335f9"
    );
    assert_eq!(size, 9);

    let retrieved_data = blob_store.read(&key).await.unwrap();
    assert_eq!(retrieved_data, data);
}

#[tokio::test]
async fn test_filesystem_blob_delete() {
    let temp_dir = tempfile::tempdir().unwrap();
    let config = Config {
        blob_dir: temp_dir.path().to_str().unwrap().to_string(),
        ..Default::default()
    };
    let store = BlobStore::from_config(&config).unwrap();

    // Deleting a missing blob is a no-op (idempotent).
    store.delete("abmissingkey.tar.gz").await.unwrap();

    // Store → read → delete → gone.
    let (key, _, _) = store.store(b"delete me").await.unwrap();
    assert!(store.read(&key).await.is_some());
    store.delete(&key).await.unwrap();
    assert!(store.read(&key).await.is_none());

    // Deleting again still succeeds.
    store.delete(&key).await.unwrap();
}
