//! Release-specific documentation captured when a version is published.
//!
//! Package-level README columns predate version pages and are intentionally not
//! copied into these fields: doing so would label current documentation as
//! documentation for an older release.

use sea_orm::DbBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Rendered Markdown can exceed MySQL `TEXT` once syntax highlighting adds
/// markup. `LONGTEXT` keeps the 512 KiB source-document limit safe there;
/// SQLite and PostgreSQL `TEXT` have no comparable 64 KiB limit.
fn document_column(name: &str, backend: DbBackend) -> ColumnDef {
    let mut column = ColumnDef::new(Alias::new(name));
    if backend == DbBackend::MySql {
        column.custom(Alias::new("LONGTEXT"));
    } else {
        column.text();
    }
    column.null();
    column
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in [
            "readme_raw",
            "readme_html",
            "changelog_raw",
            "changelog_html",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("package_versions"))
                        .add_column(document_column(column, manager.get_database_backend()))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for column in [
            "changelog_html",
            "changelog_raw",
            "readme_html",
            "readme_raw",
        ] {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("package_versions"))
                        .drop_column(Alias::new(column))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::sea_query::{MysqlQueryBuilder, PostgresQueryBuilder, SqliteQueryBuilder};

    fn add_column_sql(builder: impl SchemaBuilder) -> String {
        Table::alter()
            .table(Alias::new("package_versions"))
            .add_column(document_column("readme_html", DbBackend::MySql))
            .to_owned()
            .to_string(builder)
    }

    #[test]
    fn document_columns_use_longtext_on_mysql() {
        let sql = add_column_sql(MysqlQueryBuilder);
        assert!(sql.contains("LONGTEXT"), "{sql}");
    }

    #[test]
    fn document_columns_use_text_elsewhere() {
        for (backend, sql) in [
            (
                DbBackend::Postgres,
                Table::alter()
                    .table(Alias::new("package_versions"))
                    .add_column(document_column("readme_html", DbBackend::Postgres))
                    .to_owned()
                    .to_string(PostgresQueryBuilder),
            ),
            (
                DbBackend::Sqlite,
                Table::alter()
                    .table(Alias::new("package_versions"))
                    .add_column(document_column("readme_html", DbBackend::Sqlite))
                    .to_owned()
                    .to_string(SqliteQueryBuilder),
            ),
        ] {
            assert!(sql.to_lowercase().contains("text"), "{backend:?}: {sql}");
            assert!(!sql.contains("LONGTEXT"), "{backend:?}: {sql}");
        }
    }
}
