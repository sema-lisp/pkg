use sea_orm::{sea_query::ForeignKeyAction, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut document = ColumnDef::new(Alias::new("document_json"));
        if manager.get_database_backend() == DbBackend::MySql {
            document.custom(Alias::new("LONGTEXT"));
        } else {
            document.text();
        }
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("package_api_docs"))
                    .col(
                        ColumnDef::new(Alias::new("version_id"))
                            .big_integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(document.not_null())
                    .col(
                        ColumnDef::new(Alias::new("generated_at"))
                            .string_len(32)
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_api_docs_version")
                            .from(Alias::new("package_api_docs"), Alias::new("version_id"))
                            .to(Alias::new("package_versions"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("package_api_docs"))
                    .to_owned(),
            )
            .await
    }
}
