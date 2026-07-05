//! README storage (raw + rendered HTML) on packages. Mirrors the former
//! `006_readme.sql`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in ["readme_raw", "readme_html"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("packages"))
                        .add_column(ColumnDef::new(Alias::new(col)).text().null())
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for col in ["readme_raw", "readme_html"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("packages"))
                        .drop_column(Alias::new(col))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
