//! Full-text search acceleration for package name/description, per engine.
//!
//! - SQLite: an FTS5 external-content virtual table over `packages`, kept in
//!   sync by triggers. Search uses `MATCH` instead of scanning with `LIKE`.
//! - PostgreSQL: `pg_trgm` GIN indexes so the existing `ILIKE '%q%'` is
//!   index-accelerated. Best-effort — if the extension cannot be created the
//!   migration still succeeds and search falls back to a sequential scan.
//! - MySQL: a `FULLTEXT` index, best-effort.

use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DbBackend::Sqlite => {
                db.execute_unprepared(
                    "CREATE VIRTUAL TABLE IF NOT EXISTS packages_fts USING fts5(\
                     name, description, content='packages', content_rowid='id');",
                )
                .await?;
                db.execute_unprepared(
                    "CREATE TRIGGER IF NOT EXISTS packages_fts_ai AFTER INSERT ON packages BEGIN \
                     INSERT INTO packages_fts(rowid, name, description) \
                     VALUES (new.id, new.name, new.description); END;",
                )
                .await?;
                db.execute_unprepared(
                    "CREATE TRIGGER IF NOT EXISTS packages_fts_ad AFTER DELETE ON packages BEGIN \
                     INSERT INTO packages_fts(packages_fts, rowid, name, description) \
                     VALUES ('delete', old.id, old.name, old.description); END;",
                )
                .await?;
                db.execute_unprepared(
                    "CREATE TRIGGER IF NOT EXISTS packages_fts_au AFTER UPDATE ON packages BEGIN \
                     INSERT INTO packages_fts(packages_fts, rowid, name, description) \
                     VALUES ('delete', old.id, old.name, old.description); \
                     INSERT INTO packages_fts(rowid, name, description) \
                     VALUES (new.id, new.name, new.description); END;",
                )
                .await?;
                // Populate from any rows that already exist.
                db.execute_unprepared("INSERT INTO packages_fts(packages_fts) VALUES('rebuild');")
                    .await?;
            }
            DbBackend::Postgres => {
                // Best-effort: needs the pg_trgm extension, which some managed
                // roles cannot create. Search still works (scan) without it.
                let _ = db
                    .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm;")
                    .await;
                let _ = db
                    .execute_unprepared(
                        "CREATE INDEX IF NOT EXISTS idx_packages_name_trgm \
                         ON packages USING gin (name gin_trgm_ops);",
                    )
                    .await;
                let _ = db
                    .execute_unprepared(
                        "CREATE INDEX IF NOT EXISTS idx_packages_desc_trgm \
                         ON packages USING gin (description gin_trgm_ops);",
                    )
                    .await;
            }
            DbBackend::MySql => {
                let _ = db
                    .execute_unprepared(
                        "CREATE FULLTEXT INDEX idx_packages_ft ON packages (name, description);",
                    )
                    .await;
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DbBackend::Sqlite => {
                for stmt in [
                    "DROP TRIGGER IF EXISTS packages_fts_ai;",
                    "DROP TRIGGER IF EXISTS packages_fts_ad;",
                    "DROP TRIGGER IF EXISTS packages_fts_au;",
                    "DROP TABLE IF EXISTS packages_fts;",
                ] {
                    db.execute_unprepared(stmt).await?;
                }
            }
            DbBackend::Postgres => {
                let _ = db
                    .execute_unprepared("DROP INDEX IF EXISTS idx_packages_name_trgm;")
                    .await;
                let _ = db
                    .execute_unprepared("DROP INDEX IF EXISTS idx_packages_desc_trgm;")
                    .await;
            }
            DbBackend::MySql => {
                let _ = db
                    .execute_unprepared("DROP INDEX idx_packages_ft ON packages;")
                    .await;
            }
        }
        Ok(())
    }
}
