use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Check shared blob references without scanning every release.
        manager
            .create_index(
                Index::create()
                    .name("idx_versions_blob_key")
                    .table(Alias::new("package_versions"))
                    .col(Alias::new("blob_key"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_versions_blob_key")
                    .table(Alias::new("package_versions"))
                    .to_owned(),
            )
            .await
    }
}
