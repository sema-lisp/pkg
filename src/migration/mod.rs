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
mod m008_search;

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
        ]
    }
}
