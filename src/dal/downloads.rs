//! Daily download counters.
//!
//! The record path is an upsert expressed with SeaORM's `on_conflict` so it
//! lowers to each backend's dialect (`ON CONFLICT` on SQLite/Postgres,
//! `ON DUPLICATE KEY UPDATE` on MySQL). The date is generated in Rust.

use sea_orm::sea_query::OnConflict;
use sea_orm::{ConnectionTrait, DbErr, EntityTrait, Set};

use crate::dal::time;
use crate::entity::download_daily;

/// Increment today's download counter for `(package_name, version)`,
/// inserting the row on first download.
pub async fn record<C: ConnectionTrait>(
    db: &C,
    package_name: &str,
    version: &str,
) -> Result<(), DbErr> {
    let row = download_daily::ActiveModel {
        package_name: Set(package_name.to_string()),
        version: Set(version.to_string()),
        download_date: Set(time::today()),
        count: Set(1),
    };

    download_daily::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                download_daily::Column::PackageName,
                download_daily::Column::Version,
                download_daily::Column::DownloadDate,
            ])
            .value(
                download_daily::Column::Count,
                sea_orm::sea_query::Expr::col(download_daily::Column::Count).add(1),
            )
            .to_owned(),
        )
        .exec(db)
        .await
        .map(|_| ())
}
