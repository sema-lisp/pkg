//! Abuse-report aggregate: submission, the admin queue listing, and resolution.
//!
//! Listing joins reporters with a standard `LEFT JOIN` (bound params only) so a
//! deleted reporter surfaces as `NULL`; resolution is a single UPDATE guarded by
//! `status = 'open'` so a second resolver is a no-op. Timestamps come from
//! [`time`].

use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, Set,
    TransactionTrait,
};

use crate::dal::time;
use crate::entity::report;

/// One row of the admin report queue. `reporter` is `None` when the reporting
/// account no longer exists (the `LEFT JOIN` yields NULL).
pub struct ReportRow {
    pub id: i64,
    pub reporter: Option<String>,
    pub target_type: String,
    pub target_name: String,
    pub report_type: String,
    pub reason: String,
    pub status: String,
    pub created_at: String,
}

/// Insert a new open report, stamping `created_at` in Rust.
pub async fn create<C: ConnectionTrait>(
    db: &C,
    reporter_id: i64,
    target_type: String,
    target_name: String,
    report_type: String,
    reason: String,
) -> Result<(), DbErr> {
    let new_report = report::ActiveModel {
        id: sea_orm::NotSet,
        reporter_id: Set(reporter_id),
        target_type: Set(target_type),
        target_name: Set(target_name),
        report_type: Set(report_type),
        reason: Set(reason),
        status: Set("open".to_string()),
        resolved_by: Set(None),
        resolved_at: Set(None),
        created_at: Set(time::now()),
    };
    report::Entity::insert(new_report)
        .exec(db)
        .await
        .map(|_| ())
}

/// Count of open reports for a given target (used on the package detail view).
pub async fn count_open<C: ConnectionTrait>(db: &C, target_type: &str, target_name: &str) -> i64 {
    report::Entity::find()
        .filter(report::Column::TargetType.eq(target_type))
        .filter(report::Column::TargetName.eq(target_name))
        .filter(report::Column::Status.eq("open"))
        .count(db)
        .await
        .unwrap_or(0) as i64
}

/// List reports with the given `status`, newest first, paginated. Returns the
/// page of rows plus the total count of reports in that status.
pub async fn list<C: ConnectionTrait>(
    db: &C,
    status: &str,
    limit: i64,
    offset: i64,
) -> (Vec<ReportRow>, i64) {
    let total = report::Entity::find()
        .filter(report::Column::Status.eq(status))
        .count(db)
        .await
        .unwrap_or(0) as i64;

    let sql = format!(
        r#"SELECT r.id, u.username as reporter, r.target_type, r.target_name,
              r.report_type, r.reason, r.status, r.created_at
           FROM reports r
           LEFT JOIN users u ON u.id = r.reporter_id
           WHERE r.status = ?
           ORDER BY r.created_at DESC
           LIMIT {} OFFSET {}"#,
        limit, offset
    );
    let rows = db
        .query_all(crate::db::stmt(
            db.get_database_backend(),
            &sql,
            [status.into()],
        ))
        .await
        .unwrap_or_default();

    let reports = rows
        .iter()
        .map(|r| ReportRow {
            id: r.try_get_by::<i64, _>("id").unwrap_or(0),
            reporter: r
                .try_get_by::<Option<String>, _>("reporter")
                .unwrap_or(None),
            target_type: r.try_get_by::<String, _>("target_type").unwrap_or_default(),
            target_name: r.try_get_by::<String, _>("target_name").unwrap_or_default(),
            report_type: r.try_get_by::<String, _>("report_type").unwrap_or_default(),
            reason: r.try_get_by::<String, _>("reason").unwrap_or_default(),
            status: r.try_get_by::<String, _>("status").unwrap_or_default(),
            created_at: r.try_get_by::<String, _>("created_at").unwrap_or_default(),
        })
        .collect();

    (reports, total)
}

/// The administrative outcome of a report.
pub enum Resolution {
    Actioned,
    Dismissed,
}

/// Resolve an open report and snapshot its context in the audit log atomically.
/// Returns false when the report is missing or already resolved.
pub async fn resolve<C: ConnectionTrait + TransactionTrait>(
    db: &C,
    report_id: i64,
    resolver_id: i64,
    actor: &str,
    resolution: Resolution,
) -> Result<bool, DbErr> {
    let (status, action) = match resolution {
        Resolution::Actioned => ("actioned", "action_report"),
        Resolution::Dismissed => ("dismissed", "dismiss_report"),
    };
    let txn = db.begin().await?;
    let Some(report) = report::Entity::find_by_id(report_id).one(&txn).await? else {
        return Ok(false);
    };
    let result = txn
        .execute(crate::db::stmt(
            txn.get_database_backend(),
            "UPDATE reports SET status = ?, resolved_by = ?, resolved_at = ? WHERE id = ? AND status = 'open'",
            [
                status.into(),
                resolver_id.into(),
                time::now().into(),
                report_id.into(),
            ],
        ))
        .await?;
    if result.rows_affected() == 0 {
        return Ok(false);
    }
    super::audit_log::record(
        &txn,
        actor,
        action,
        Some("report"),
        Some(&report_id.to_string()),
        Some(&crate::audit::report_detail(&report, status)),
    )
    .await?;
    txn.commit().await?;
    Ok(true)
}
