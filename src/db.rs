use crate::migration::Migrator;
use sea_orm::{ConnectionTrait, Database, DatabaseBackend, DatabaseConnection, Statement};
use sea_orm_migration::MigratorTrait;

pub type Db = DatabaseConnection;

/// A helper to build a Statement from raw SQL, automatically translating
/// '?' placeholders to Postgres '$1', '$2', etc., when the backend is Postgres.
pub fn stmt<I>(backend: DatabaseBackend, sql: &str, values: I) -> Statement
where
    I: IntoIterator<Item = sea_orm::Value>,
{
    let sql = if backend == DatabaseBackend::Postgres {
        let mut result = String::new();
        let mut placeholder_idx = 1;
        for part in sql.split('?') {
            if !result.is_empty() {
                result.push_str(&format!("${placeholder_idx}"));
                placeholder_idx += 1;
            }
            result.push_str(part);
        }
        result
    } else {
        sql.to_string()
    };
    Statement::from_sql_and_values(backend, sql, values)
}

/// Connect to the registry database and bring the schema up to date.
///
/// The engine is inferred from the URL scheme (`sqlite:` / `postgres:` /
/// `mysql:`), so the same binary runs on all three. Schema is applied via the
/// engine-agnostic SeaORM migrations in [`crate::migration`].
pub async fn connect(database_url: &str) -> Db {
    // An in-memory SQLite DB lives inside a single connection, so the whole
    // pool must share one connection or migrations and queries would land in
    // different (empty) databases.
    let in_memory = database_url.contains(":memory:");

    // Pool size caps read concurrency; tune with DATABASE_MAX_CONNECTIONS. For
    // SQLite, bigger is not always better — WAL readers run concurrently but
    // share CPU, so oversubscribing hurts on expensive queries. An in-memory DB
    // must stay at one shared connection or each sees a different (empty) DB.
    let max = std::env::var("DATABASE_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse::<u32>().ok())
        .filter(|&n| n > 0)
        .unwrap_or(10);

    let mut opts = sea_orm::ConnectOptions::new(database_url.to_string());
    opts.max_connections(if in_memory { 1 } else { max })
        .min_connections(1);

    let db = Database::connect(opts)
        .await
        .expect("Failed to connect to database");

    // WAL improves concurrent read/write on SQLite file databases; it is a
    // SQLite-only pragma and simply does not apply to Postgres/MySQL.
    if db.get_database_backend() == DatabaseBackend::Sqlite {
        let _ = db.execute_unprepared("PRAGMA journal_mode=WAL;").await;
    }

    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    db
}

pub fn row_get_i64(row: &sea_orm::QueryResult, col: &str) -> Option<i64> {
    if let Ok(v) = row.try_get::<i64>("", col) {
        return Some(v);
    }
    if let Ok(v) = row.try_get::<sea_orm::prelude::Decimal>("", col) {
        if let Ok(n) = v.to_string().parse::<f64>() {
            return Some(n as i64);
        }
    }
    if let Ok(v) = row.try_get::<f64>("", col) {
        return Some(v as i64);
    }
    if let Ok(v) = row.try_get::<i32>("", col) {
        return Some(v as i64);
    }
    None
}
