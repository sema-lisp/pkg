use crate::db::Db;

/// Snapshot the report's subject and reason so its audit entry remains useful
/// after the report or package is deleted.
pub fn report_detail(report: &crate::entity::report::Model, outcome: &str) -> String {
    format!(
        "{} {} · {} · {}\nReason: {}",
        report.target_type, report.target_name, report.report_type, outcome, report.reason
    )
}

/// Record an imported webhook tag against the package's registry name.
pub(crate) async fn log_webhook_sync(
    db: &Db,
    package: &crate::entity::package::Model,
    repository: &str,
    tag: &str,
    version: &semver::Version,
) {
    log(
        db,
        "system",
        "webhook_sync",
        Some("package"),
        Some(&package.name),
        Some(&format!(
            "Repository: {repository} · Tag: {tag} · Version: {version}"
        )),
    )
    .await;
}

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

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn webhook_audit_distinguishes_package_name_from_repository() {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::db::connect(&format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("audit.db").display()
        ))
        .await;
        let package = crate::dal::packages::create_github(
            &db,
            "manifest-name",
            "",
            None,
            "acme/repo-name",
            "test-secret",
        )
        .await
        .unwrap();
        super::log_webhook_sync(
            &db,
            &package,
            "acme/repo-name",
            "v1.2.3",
            &semver::Version::new(1, 2, 3),
        )
        .await;
        let (entries, total) = crate::dal::audit_log::list(
            &db,
            &crate::dal::audit_log::AuditFilter {
                q: Some("manifest-name".into()),
                action: Some("webhook_sync".into()),
                limit: 50,
                offset: 0,
            },
        )
        .await;
        assert_eq!(total, 1);
        assert_eq!(entries[0].actor, "system");
        assert_eq!(entries[0].target_type.as_deref(), Some("package"));
        assert_eq!(entries[0].target_name.as_deref(), Some("manifest-name"));
        assert_eq!(
            entries[0].detail.as_deref(),
            Some("Repository: acme/repo-name · Tag: v1.2.3 · Version: 1.2.3")
        );
    }
}
