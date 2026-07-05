//! Initial schema: users, sessions, api_tokens, packages, package_versions,
//! dependencies, owners. Mirrors the former `001_initial.sql`.
//!
//! Portability notes:
//! - Indexed / unique / primary-key / compared string columns are sized
//!   `VARCHAR` (MySQL cannot index or key a `TEXT` column); only large,
//!   unindexed free text (descriptions, etc.) uses `TEXT`.
//! - Timestamps are `VARCHAR(32)` holding canonical `YYYY-MM-DD HH:MM:SS`
//!   strings the app writes via `dal::time` — no engine-specific defaults.
//! - Foreign keys are intentionally not declared: the original SQLite schema
//!   ran with enforcement off, so we keep identical semantics rather than
//!   introduce newly-enforced constraints on Postgres/MySQL.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Timestamp column width (`YYYY-MM-DD HH:MM:SS` is 19 chars).
const TS: u32 = 32;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Alias::new("users"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("username"))
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("email"))
                            .string_len(255)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("password_hash"))
                            .string_len(255)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("github_id"))
                            .big_integer()
                            .null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Alias::new("homepage")).text().null())
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("sessions"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("expires_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("api_tokens"))
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
                        ColumnDef::new(Alias::new("name"))
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("token_hash"))
                            .string_len(128)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("scopes"))
                            .string_len(64)
                            .not_null()
                            .default("publish"),
                    )
                    .col(
                        ColumnDef::new(Alias::new("last_used_at"))
                            .string_len(TS)
                            .null(),
                    )
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
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("packages"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("name"))
                            .string_len(160)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("description"))
                            .string_len(2048)
                            .not_null()
                            .default(""),
                    )
                    .col(
                        ColumnDef::new(Alias::new("repository_url"))
                            .string_len(512)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("created_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("package_versions"))
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
                    .col(
                        ColumnDef::new(Alias::new("version"))
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("checksum_sha256"))
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("blob_key"))
                            .string_len(160)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("size_bytes"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("yanked"))
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Alias::new("sema_version_req"))
                            .string_len(128)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("published_at"))
                            .string_len(TS)
                            .not_null(),
                    )
                    .index(
                        Index::create()
                            .name("uq_versions_package_version")
                            .col(Alias::new("package_id"))
                            .col(Alias::new("version"))
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("dependencies"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("id"))
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("version_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("dependency_name"))
                            .string_len(160)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("version_req"))
                            .string_len(128)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Alias::new("owners"))
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Alias::new("package_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Alias::new("user_id"))
                            .big_integer()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(Alias::new("package_id"))
                            .col(Alias::new("user_id")),
                    )
                    .to_owned(),
            )
            .await?;

        for (name, table, col) in [
            ("idx_versions_package", "package_versions", "package_id"),
            ("idx_deps_version", "dependencies", "version_id"),
            ("idx_tokens_user", "api_tokens", "user_id"),
            ("idx_sessions_user", "sessions", "user_id"),
        ] {
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name(name)
                        .table(Alias::new(table))
                        .col(Alias::new(col))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "owners",
            "dependencies",
            "package_versions",
            "packages",
            "api_tokens",
            "sessions",
            "users",
        ] {
            manager
                .drop_table(
                    Table::drop()
                        .table(Alias::new(table))
                        .if_exists()
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
