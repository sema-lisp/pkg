//! Audit log. [`record`] appends an entry; [`list`] reads the trail back for
//! the admin console. [`crate::audit::log`] wraps `record` with failure-logging.
//!
//! The listing builds its WHERE clause from static fragment literals, pushing
//! every user value as a bound parameter, then interpolates only the assembled
//! fragment string into the query — injection-safe and portable across engines.

use sea_orm::{
    ActiveModelTrait, ConnectionTrait, DatabaseBackend, DbErr, FromQueryResult, Set, Value,
};

use crate::dal::time;
use crate::entity::audit_log;

/// Append one audit-trail entry (stamps `created_at`).
pub async fn record<C: ConnectionTrait>(
    db: &C,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_name: Option<&str>,
    detail: Option<&str>,
) -> Result<(), sea_orm::DbErr> {
    let model = audit_log::ActiveModel {
        actor: Set(actor.to_string()),
        action: Set(action.to_string()),
        target_type: Set(target_type.map(String::from)),
        target_name: Set(target_name.map(String::from)),
        detail: Set(detail.map(String::from)),
        created_at: Set(time::now()),
        ..Default::default()
    };
    model.insert(db).await.map(|_| ())
}

/// Filters for [`list`]. `q` matches actor/target/detail via `LIKE`; `action`
/// is an exact match. `limit`/`offset` paginate.
pub struct AuditFilter {
    pub q: Option<String>,
    pub action: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

/// List audit entries matching `filter`, newest first. Returns the page of rows
/// plus the total number of matching entries.
pub async fn list<C: ConnectionTrait>(
    db: &C,
    filter: &AuditFilter,
) -> Result<(Vec<audit_log::Model>, i64), DbErr> {
    let mut where_clauses = Vec::new();
    let mut binds: Vec<Value> = Vec::new();

    if let Some(ref action) = filter.action {
        where_clauses.push("action = ?");
        binds.push(action.clone().into());
    }

    if let Some(ref q) = filter.q {
        let pattern = format!("%{q}%");
        where_clauses.push("(actor LIKE ? OR target_name LIKE ? OR detail LIKE ?)");
        binds.push(pattern.clone().into());
        binds.push(pattern.clone().into());
        binds.push(pattern.into());
    }

    // An unfiltered COUNT(*) can use SQLite's fast count operation. Even
    // WHERE 1=1 prevents that optimization and walks every index entry.
    let where_sql = if where_clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_clauses.join(" AND "))
    };

    let count_sql = format!("SELECT COUNT(*) FROM audit_log{where_sql}");
    let total: i64 = match db
        .query_one(crate::db::stmt(
            db.get_database_backend(),
            &count_sql,
            binds.clone(),
        ))
        .await?
    {
        Some(row) => row.try_get_by_index(0)?,
        None => 0,
    };
    if filter.offset >= total {
        return Ok((Vec::new(), total));
    }

    // If all text matches fit on one page, SQLite's date index can still walk
    // the entire history and fetch every row to check the text. Scan the table
    // directly in that case; otherwise let the ordered index stop at LIMIT.
    let index_hint = if db.get_database_backend() == DatabaseBackend::Sqlite
        && filter.q.is_some()
        && filter.action.is_none()
        && total <= filter.limit
    {
        " NOT INDEXED"
    } else {
        ""
    };
    let sql = format!(
        r#"SELECT id, actor, action, target_type, target_name, detail, created_at
           FROM audit_log{index_hint}{where_sql}
           ORDER BY created_at DESC, id DESC
           LIMIT ? OFFSET ?"#
    );

    let mut all_binds = binds;
    all_binds.push(filter.limit.into());
    all_binds.push(filter.offset.into());

    let entries = audit_log::Model::find_by_statement(crate::db::stmt(
        db.get_database_backend(),
        &sql,
        all_binds,
    ))
    .all(db)
    .await?;

    Ok((entries, total))
}
