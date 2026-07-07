//! Covering index for the dashboard's rolling download total.
//!
//! `SELECT SUM(count) FROM download_daily WHERE download_date >= ?` filtered on
//! the date index but then fetched `count` per row — over a million row lookups
//! for a 30-day window at scale. A composite `(download_date, count)` index lets
//! the sum run index-only.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_download_daily_date_count")
                    .table(Alias::new("download_daily"))
                    .col(Alias::new("download_date"))
                    .col(Alias::new("count"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_download_daily_date_count")
                    .table(Alias::new("download_daily"))
                    .to_owned(),
            )
            .await
    }
}
