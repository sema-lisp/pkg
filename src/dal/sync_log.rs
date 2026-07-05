//! GitHub tag-sync audit log (`github_sync_log`).
//!
//! Each sync attempt records one row: `ok` on a successful import, `error`
//! with a message otherwise. `synced_at` is application-generated via [`time`].

use sea_orm::{ActiveModelTrait, ConnectionTrait, DbErr, Set};

use crate::dal::time;
use crate::entity::github_sync_log;

/// Record a successful tag sync.
pub async fn record_ok<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    tag: &str,
) -> Result<(), DbErr> {
    let row = github_sync_log::ActiveModel {
        package_id: Set(package_id),
        tag: Set(tag.to_string()),
        status: Set("ok".into()),
        synced_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await.map(|_| ())
}

/// Record a failed tag sync with its error message.
pub async fn record_error<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    tag: &str,
    error: &str,
) -> Result<(), DbErr> {
    let row = github_sync_log::ActiveModel {
        package_id: Set(package_id),
        tag: Set(tag.to_string()),
        status: Set("error".into()),
        error: Set(Some(error.to_string())),
        synced_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await.map(|_| ())
}
