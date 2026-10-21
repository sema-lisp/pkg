//! Engine-agnostic schema migrations (SeaORM programmatic migrations).
//!
//! These replace the former SQLite-dialect `migrations/*.sql` files so the
//! registry can boot on SQLite, PostgreSQL, and MySQL from one source of
//! truth. Each migration mirrors one of the original `.sql` files; the schema
//! is identical except that timestamp/date columns are plain **TEXT**. The
//! application always writes canonical `YYYY-MM-DD HH:MM:SS` strings
//! (`dal::time`) rather than relying on per-engine `CURRENT_TIMESTAMP`
//! defaults, so text columns behave identically everywhere.

use sea_orm_migration::prelude::*;

mod m001_initial;
mod m002_github_packages;
mod m003_meta_registry;
mod m004_admin;
mod m005_download_daily;
mod m006_readme;
mod m007_perf_indexes;
// Public so the dev seeder can drop/rebuild the FTS index around a bulk load.
pub mod m008_search;
mod m009_download_sum_index;
mod m010_audit_action_order;
mod m010_release_metadata;
mod m011_version_blob_index;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m001_initial::Migration),
            Box::new(m002_github_packages::Migration),
            Box::new(m003_meta_registry::Migration),
            Box::new(m004_admin::Migration),
            Box::new(m005_download_daily::Migration),
            Box::new(m006_readme::Migration),
            Box::new(m007_perf_indexes::Migration),
            Box::new(m008_search::Migration),
            Box::new(m009_download_sum_index::Migration),
            Box::new(m010_release_metadata::Migration),
            Box::new(m010_audit_action_order::Migration),
            Box::new(m011_version_blob_index::Migration),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ConnectionTrait, Database};

    #[tokio::test]
    async fn either_feature_migration_can_be_applied_first() {
        for first in ["m010_release_metadata", "m010_audit_action_order"] {
            let db = Database::connect("sqlite::memory:").await.unwrap();
            Migrator::up(&db, Some(9)).await.unwrap();
            let migration = Migrator::migrations()
                .into_iter()
                .find(|migration| migration.name() == first)
                .unwrap();
            migration.up(&SchemaManager::new(&db)).await.unwrap();
            db.execute(crate::db::stmt(
                db.get_database_backend(),
                "INSERT INTO seaql_migrations (version, applied_at) VALUES (?, ?)",
                [first.into(), 0i64.into()],
            ))
            .await
            .unwrap();
            Migrator::up(&db, None).await.unwrap();
            // A second startup must not try to add either schema change again.
            Migrator::up(&db, None).await.unwrap();
            let manager = SchemaManager::new(&db);
            assert!(manager
                .has_column("package_versions", "readme_html")
                .await
                .unwrap());
            assert!(manager
                .has_index("audit_log", "idx_audit_log_action_order")
                .await
                .unwrap());
        }
    }
}
