//! Daily download aggregates per package+version. Mirrors the former
//! `005_download_daily.sql`. The composite unique enables the download-count
//! upsert (SeaORM `on_conflict`, lowered per backend).

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Retire any legacy per-download event log if present.
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("download_log"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("download_daily"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("package_name"))
                            .string_len(160)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("version"))
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("download_date"))
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("count"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(
                        Index::create()
                            .col(Alias::new("package_name"))
                            .col(Alias::new("version"))
                            .col(Alias::new("download_date")),
                    )
                    .index(
                        Index::create()
                            .name("uq_download_daily")
                            .col(Alias::new("package_name"))
                            .col(Alias::new("version"))
                            .col(Alias::new("download_date"))
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        for (name, col) in [
            ("idx_download_daily_pkg", "package_name"),
            ("idx_download_daily_date", "download_date"),
        ] {
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name(name)
                        .table(Alias::new("download_daily"))
                        .col(Alias::new(col))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("download_daily"))
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}
