//! Secondary indexes for the admin and listing read paths. Plain single-column
//! indexes, portable across SQLite/Postgres/MySQL.
//!
//! - `owners(user_id)`: the admin user listing counts a user's packages with a
//!   correlated `COUNT(*) FROM owners WHERE user_id = ?`. The table's PK is
//!   `(package_id, user_id)`, so a lookup by `user_id` alone cannot use it;
//!   this index makes it a point lookup instead of a full table scan per row.
//! - `users(created_at)` / `packages(created_at)`: the admin/user and package
//!   listings `ORDER BY created_at DESC`; the index avoids a full sort.
//! - `package_versions(published_at)`: the homepage orders recent releases by
//!   `published_at DESC`.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const INDEXES: &[(&str, &str, &str)] = &[
    ("idx_owners_user", "owners", "user_id"),
    ("idx_users_created", "users", "created_at"),
    ("idx_packages_created", "packages", "created_at"),
    ("idx_versions_published", "package_versions", "published_at"),
];

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, table, col) in INDEXES {
            manager
                .create_index(
                    Index::create()
                        .if_not_exists()
                        .name(*name)
                        .table(Alias::new(*table))
                        .col(Alias::new(*col))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for (name, table, _) in INDEXES {
            manager
                .drop_index(
                    Index::drop()
                        .if_exists()
                        .name(*name)
                        .table(Alias::new(*table))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
