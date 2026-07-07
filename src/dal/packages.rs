//! Package aggregate: lookups, creation, description updates, and search.
//!
//! Search uses SQLite FTS5 (`MATCH`) when available and falls back to `LIKE`
//! with bound patterns on other backends (Postgres accelerates the same `LIKE`
//! via `pg_trgm`); `created_at` is application-generated via [`time`].

use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseBackend, DbErr,
    EntityTrait, PaginatorTrait, QueryFilter, Set,
};

use crate::dal::time;
use crate::entity::{owner, package, package_version, report};

/// A search predicate over the `packages` table (aliased `p`): the join to add
/// to the FROM clause, the WHERE fragment, and its bound values, chosen per
/// backend. An empty query matches all rows.
///
/// - SQLite: FTS5 `MATCH` (quoted prefix; embedded quotes doubled for safety).
/// - MySQL: `MATCH … AGAINST` boolean mode, falling back to `LIKE` when the
///   query has no indexable tokens.
/// - Postgres / other: `LIKE '%q%'` (Postgres accelerates it via `pg_trgm`).
fn search_predicate(
    backend: DatabaseBackend,
    q: &str,
) -> (&'static str, String, Vec<sea_orm::Value>) {
    if q.trim().is_empty() {
        return ("", "1=1".into(), vec![]);
    }
    let like = || {
        let pat = format!("%{q}%");
        (
            "",
            "(p.name LIKE ? OR p.description LIKE ?)".to_string(),
            vec![pat.clone().into(), pat.into()],
        )
    };
    match backend {
        DatabaseBackend::Sqlite => {
            let m = format!("\"{}\"*", q.replace('"', "\"\""));
            (
                "JOIN packages_fts f ON f.rowid = p.id",
                "packages_fts MATCH ?".into(),
                vec![m.into()],
            )
        }
        DatabaseBackend::MySql => {
            let expr = mysql_boolean(q);
            if expr.is_empty() {
                like()
            } else {
                (
                    "",
                    "MATCH(p.name, p.description) AGAINST (? IN BOOLEAN MODE)".into(),
                    vec![expr.into()],
                )
            }
        }
        _ => like(),
    }
}

/// A boolean-mode FULLTEXT expression: each alphanumeric token required as a
/// prefix (`+token*`). Empty when the query has no indexable tokens.
fn mysql_boolean(q: &str) -> String {
    q.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| format!("+{t}*"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Relevance `ORDER BY`: exact name match first, then a name prefix, then the
/// rest, breaking ties alphabetically. Returns the fragment and its binds (which
/// follow the WHERE binds). An empty query just orders by name.
fn relevance_order(q: &str) -> (String, Vec<sea_orm::Value>) {
    if q.trim().is_empty() {
        return ("p.name".into(), vec![]);
    }
    (
        "CASE WHEN p.name = ? THEN 0 WHEN p.name LIKE ? THEN 1 ELSE 2 END, p.name".into(),
        vec![q.into(), format!("{q}%").into()],
    )
}

/// Look up a package by its unique name.
pub async fn find_by_name<C: ConnectionTrait>(
    db: &C,
    name: &str,
) -> Result<Option<package::Model>, DbErr> {
    package::Entity::find()
        .filter(package::Column::Name.eq(name))
        .one(db)
        .await
}

/// Total number of packages.
pub async fn count<C: ConnectionTrait>(db: &C) -> i64 {
    package::Entity::find().count(db).await.unwrap_or(0) as i64
}

/// A package summary row for server-rendered listings:
/// `(name, description, latest_version, published_at)`.
pub type ListingRow = (String, String, String, String);

fn listing_row(r: &sea_orm::QueryResult) -> ListingRow {
    (
        r.try_get("", "name").unwrap_or_default(),
        r.try_get("", "description").unwrap_or_default(),
        r.try_get("", "latest_version").unwrap_or_default(),
        r.try_get("", "published_at").unwrap_or_default(),
    )
}

/// The most recently published packages, newest first, each paired with its
/// latest version (highest-id row). `limit` bounds the result. Used by the
/// homepage.
pub async fn recent<C: ConnectionTrait>(db: &C, limit: i64) -> Vec<ListingRow> {
    let sql = format!(
        r#"SELECT p.name, p.description, pv.version AS latest_version, pv.published_at
           FROM packages p
           JOIN package_versions pv ON pv.package_id = p.id
           WHERE pv.id = (SELECT MAX(pv2.id) FROM package_versions pv2 WHERE pv2.package_id = p.id)
           ORDER BY pv.published_at DESC
           LIMIT {}"#,
        limit
    );
    let rows = db
        .query_all(crate::db::stmt(
            db.get_database_backend(),
            &sql,
            Vec::<sea_orm::Value>::new(),
        ))
        .await
        .unwrap_or_default();
    rows.iter().map(listing_row).collect()
}

/// Search packages by name/description for the server-rendered search page,
/// resolving each hit's latest version and publish time via correlated
/// subqueries. Differs from [`search`] (the JSON API), which returns
/// `created_at` instead.
#[tracing::instrument(skip_all, level = "debug")]
pub async fn search_page<C: ConnectionTrait>(
    db: &C,
    q: &str,
    limit: i64,
    offset: i64,
) -> Vec<ListingRow> {
    let backend = db.get_database_backend();
    let (join, where_sql, mut binds) = search_predicate(backend, q);
    let (order, order_binds) = relevance_order(q);
    binds.extend(order_binds);
    let sql = format!(
        r#"SELECT p.name, p.description,
           COALESCE((SELECT pv.version FROM package_versions pv WHERE pv.package_id = p.id ORDER BY pv.id DESC LIMIT 1), '') as latest_version,
           COALESCE((SELECT pv.published_at FROM package_versions pv WHERE pv.package_id = p.id ORDER BY pv.id DESC LIMIT 1), p.created_at) as published_at
           FROM packages p {join}
           WHERE {where_sql}
           ORDER BY {order}
           LIMIT {limit} OFFSET {offset}"#
    );
    let rows = db
        .query_all(crate::db::stmt(backend, &sql, binds))
        .await
        .unwrap_or_default();
    rows.iter().map(listing_row).collect()
}

/// Packages owned by `user_id` for the account page, each with its latest
/// version and publish time, alphabetical by name.
pub async fn list_for_owner<C: ConnectionTrait>(db: &C, user_id: i64) -> Vec<ListingRow> {
    let rows = db
        .query_all(crate::db::stmt(
            db.get_database_backend(),
            r#"SELECT p.name, p.description,
               COALESCE((SELECT pv.version FROM package_versions pv WHERE pv.package_id = p.id ORDER BY pv.id DESC LIMIT 1), '') as latest_version,
               COALESCE((SELECT pv.published_at FROM package_versions pv WHERE pv.package_id = p.id ORDER BY pv.id DESC LIMIT 1), p.created_at) as published_at
               FROM packages p
               JOIN owners o ON o.package_id = p.id
               WHERE o.user_id = ?
               ORDER BY p.name"#,
            [user_id.into()],
        ))
        .await
        .unwrap_or_default();
    rows.iter().map(listing_row).collect()
}

/// Find a GitHub-linked package by its `owner/repo` full name (only rows with
/// `source = 'github'`).
pub async fn find_github_linked<C: ConnectionTrait>(
    db: &C,
    repo_full_name: &str,
) -> Result<Option<package::Model>, DbErr> {
    package::Entity::find()
        .filter(package::Column::GithubRepo.eq(repo_full_name))
        .filter(package::Column::Source.eq("github"))
        .one(db)
        .await
}

/// Resolve `(id, source, github_repo)` for a package by name, but only if
/// `user_id` is one of its owners. `github_repo` is empty when unset.
pub async fn find_owned<C: ConnectionTrait>(
    db: &C,
    name: &str,
    user_id: i64,
) -> Option<(i64, String, String)> {
    let row = db
        .query_one(crate::db::stmt(
            db.get_database_backend(),
            r#"SELECT p.id, p.source, p.github_repo FROM packages p
               JOIN owners o ON o.package_id = p.id
               WHERE p.name = ? AND o.user_id = ?"#,
            [name.into(), user_id.into()],
        ))
        .await
        .ok()
        .flatten()?;
    Some((
        row.try_get("", "id").unwrap_or_default(),
        row.try_get("", "source").unwrap_or_default(),
        row.try_get("", "github_repo").unwrap_or_default(),
    ))
}

/// Insert a new upload-sourced package, stamping `created_at` in Rust.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    name: &str,
    description: &str,
    repository_url: Option<String>,
) -> Result<package::Model, DbErr> {
    let row = package::ActiveModel {
        name: Set(name.to_string()),
        description: Set(description.to_string()),
        repository_url: Set(repository_url),
        source: Set("upload".into()),
        created_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await
}

/// Insert a new GitHub-sourced package (created via repo link), stamping
/// `created_at` in Rust. Returns the inserted row.
pub async fn create_github<C: ConnectionTrait>(
    db: &C,
    name: &str,
    description: &str,
    repository_url: Option<String>,
    github_repo: &str,
    webhook_secret: &str,
) -> Result<package::Model, DbErr> {
    let row = package::ActiveModel {
        name: Set(name.to_string()),
        description: Set(description.to_string()),
        repository_url: Set(repository_url),
        source: Set("github".into()),
        github_repo: Set(Some(github_repo.to_string())),
        webhook_secret: Set(Some(webhook_secret.to_string())),
        created_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await
}

/// Store the raw + rendered README for a package. A no-op if the package no
/// longer exists.
pub async fn set_readme<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    readme_raw: &str,
    readme_html: &str,
) -> Result<(), DbErr> {
    package::Entity::update_many()
        .col_expr(package::Column::ReadmeRaw, Expr::value(readme_raw))
        .col_expr(package::Column::ReadmeHtml, Expr::value(readme_html))
        .filter(package::Column::Id.eq(package_id))
        .exec(db)
        .await
        .map(|_| ())
}

/// Overwrite a package's description.
pub async fn update_description<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    description: &str,
) -> Result<(), DbErr> {
    package::Entity::update_many()
        .col_expr(package::Column::Description, Expr::value(description))
        .filter(package::Column::Id.eq(package_id))
        .exec(db)
        .await
        .map(|_| ())
}

/// Admin: yank every version of a package by name. The subquery resolves the
/// package id inside the UPDATE so it stays a single portable statement.
/// Returns the number of versions yanked (0 if the package is missing or has
/// no versions).
pub async fn yank_all<C: ConnectionTrait>(db: &C, name: &str) -> u64 {
    let result = db
        .execute(crate::db::stmt(
            db.get_database_backend(),
            "UPDATE package_versions SET yanked = 1 WHERE package_id = (SELECT id FROM packages WHERE name = ?)",
            [name.into()],
        ))
        .await;
    result.map(|r| r.rows_affected()).unwrap_or(0)
}

/// Admin: delete a package and everything hanging off it — dependencies (via a
/// version_id subquery), versions, owners, the package row, and any reports
/// targeting it. Best-effort per step, mirroring the original handler. Returns
/// `false` if no package with `name` exists (so the caller can 404), `true`
/// once the cascade has run.
pub async fn delete_by_name<C: ConnectionTrait>(db: &C, name: &str) -> bool {
    let pkg_id = match find_by_name(db, name).await.ok().flatten() {
        Some(p) => p.id,
        None => return false,
    };

    // Delete dependencies via version_id join (raw SQL for subquery)
    let _ = db
        .execute(crate::db::stmt(
            db.get_database_backend(),
            "DELETE FROM dependencies WHERE version_id IN (SELECT id FROM package_versions WHERE package_id = ?)",
            [pkg_id.into()],
        ))
        .await;

    // Delete versions
    let _ = package_version::Entity::delete_many()
        .filter(package_version::Column::PackageId.eq(pkg_id))
        .exec(db)
        .await;

    // Delete owners
    let _ = owner::Entity::delete_many()
        .filter(owner::Column::PackageId.eq(pkg_id))
        .exec(db)
        .await;

    // Delete the package
    let _ = package::Entity::delete_by_id(pkg_id).exec(db).await;

    // Clean up any reports targeting this package
    let _ = report::Entity::delete_many()
        .filter(report::Column::TargetType.eq("package"))
        .filter(report::Column::TargetName.eq(name))
        .exec(db)
        .await;

    true
}

/// A single search hit: `(name, description, created_at)`.
pub type SearchHit = (String, String, String);

/// Search packages whose name or description matches `q`, ordered by name,
/// paginated with `limit`/`offset`.
#[tracing::instrument(skip_all, level = "debug")]
pub async fn search<C: ConnectionTrait>(
    db: &C,
    q: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<SearchHit>, DbErr> {
    let backend = db.get_database_backend();
    let (join, where_sql, mut binds) = search_predicate(backend, q);
    let (order, order_binds) = relevance_order(q);
    binds.extend(order_binds);
    let sql = format!(
        r#"SELECT p.name, p.description, p.created_at FROM packages p {join}
           WHERE {where_sql}
           ORDER BY {order}
           LIMIT {limit} OFFSET {offset}"#
    );
    let rows = db.query_all(crate::db::stmt(backend, &sql, binds)).await?;

    Ok(rows
        .iter()
        .filter_map(|r| {
            let name: String = r.try_get("", "name").ok()?;
            let description: String = r.try_get("", "description").ok()?;
            let created_at: String = r.try_get("", "created_at").ok()?;
            Some((name, description, created_at))
        })
        .collect())
}

/// Count packages matching the same predicate as [`search`].
#[tracing::instrument(skip_all, level = "debug")]
pub async fn search_count<C: ConnectionTrait>(db: &C, q: &str) -> Result<i64, DbErr> {
    let backend = db.get_database_backend();
    let (join, where_sql, binds) = search_predicate(backend, q);
    let sql = format!("SELECT COUNT(*) as cnt FROM packages p {join} WHERE {where_sql}");
    let row = db.query_one(crate::db::stmt(backend, &sql, binds)).await?;
    Ok(row.and_then(|r| r.try_get("", "cnt").ok()).unwrap_or(0))
}
