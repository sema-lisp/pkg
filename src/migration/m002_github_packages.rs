//! OAuth connections + GitHub-package source tracking + sync log.
//! Mirrors the former `002_github_packages.sql`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const TS: u32 = 32;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("oauth_connections"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("provider"))
                            .string_len(32)
                            .not_null()
                            .default("github"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("provider_user_id"))
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("provider_login"))
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("access_token_enc"))
                            .binary()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("scopes")).string_len(255).null())
                    .col(
                        ColumnDef::new(Alias::new("revoked_at"))
                            .string_len(TS)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("updated_at"))
                            .string_len(TS)
                            .null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth_provider_user")
                    .table(Alias::new("oauth_connections"))
                    .col(Alias::new("provider"))
                    .col(Alias::new("provider_user_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_oauth_user_provider")
                    .table(Alias::new("oauth_connections"))
                    .col(Alias::new("user_id"))
                    .col(Alias::new("provider"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        for (col, ty) in [
            ("source", 32u32),
            ("github_repo", 255),
            ("webhook_secret", 128),
        ] {
            let mut def = ColumnDef::new(Alias::new(col));
            def.string_len(ty);
            if col == "source" {
                def.not_null().default("upload");
            } else {
                def.null();
            }
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("packages"))
                        .add_column(&mut def)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("github_sync_log"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("package_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("tag")).string_len(128).not_null())
                    .col(
                        ColumnDef::new(Alias::new("status"))
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Alias::new("error")).text().null())
                    .col(
                        ColumnDef::new(Alias::new("synced_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sync_log_package")
                    .table(Alias::new("github_sync_log"))
                    .col(Alias::new("package_id"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("github_sync_log"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        for col in ["source", "github_repo", "webhook_secret"] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("packages"))
                        .drop_column(Alias::new(col))
                        .to_owned(),
                )
                .await?;
        }
        manager
            .drop_table(
                Table::drop()
                    .table(Alias::new("oauth_connections"))
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}
