//! GitHub-linked packages store an upstream redirect URL instead of a blob.
//! Mirrors the former `003_meta_registry.sql`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("package_versions"))
                    .add_column(
                        ColumnDef::new(Alias::new("tarball_url"))
                            .string_len(512)
                            .null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("package_versions"))
                    .drop_column(Alias::new("tarball_url"))
                    .to_owned(),
            )
            .await
    }
}
