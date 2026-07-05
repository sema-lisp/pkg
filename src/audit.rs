use crate::db::Db;

/// Log an action to the audit trail. On failure, emits a tracing::error
/// so audit failures are always observable even if the caller continues.
///
/// - `actor`: username or "system"
/// - `action`: verb (e.g. "publish", "ban", "yank", "register")
/// - `target_type`: optional — "user", "package", or "version"
/// - `target_name`: optional — the username or package name affected
/// - `detail`: optional — free-text description, reason, version string, etc.
pub async fn log(
    db: &Db,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_name: Option<&str>,
    detail: Option<&str>,
) {
    if let Err(e) =
        crate::dal::audit_log::record(db, actor, action, target_type, target_name, detail).await
    {
        tracing::error!(
            error = %e,
            actor = actor,
            action = action,
            "AUDIT LOG FAILED — action was performed but not recorded"
        );
    }
}
