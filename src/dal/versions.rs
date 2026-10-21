//! Package-version aggregate: listing, existence, creation, download
//! resolution, and yanking.
//!
//! `published_at` is application-generated via [`time`]; the download-target
//! join is standard SQL with bound params.

use sea_orm::{
    sea_query::Expr, ActiveModelTrait, ColumnTrait, ConnectionTrait, DbErr, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set,
};

use crate::dal::time;
use crate::entity::package_version;

/// Documentation captured from one release when it is published. Empty values
/// stay absent so legacy rows never pretend to document a historical release.
#[derive(Clone, Debug, Default)]
pub struct ReleaseDocuments {
    pub readme_raw: Option<String>,
    pub readme_html: Option<String>,
    pub changelog_raw: Option<String>,
    pub changelog_html: Option<String>,
}

/// The small release projection used in lists. It deliberately excludes the
/// rendered documents, which can be large and are only needed for one page.
#[derive(Clone, Debug)]
pub struct ReleaseCandidate {
    pub id: i64,
    pub package_id: i64,
    pub version: String,
    pub size_bytes: i64,
    pub checksum_sha256: String,
    pub yanked: i32,
    pub sema_version_req: Option<String>,
    pub tarball_url: Option<String>,
    pub published_at: String,
}

/// The bounded projection used by the release-documentation backfill. It
/// deliberately excludes stored documentation, which can be much larger than
/// the archive metadata needed to verify and process each version.
#[derive(Clone, Debug)]
pub struct DocumentBackfillTarget {
    pub id: i64,
    pub blob_key: String,
    pub checksum_sha256: String,
    pub tarball_url: Option<String>,
}

/// The release selected for a package's default page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefaultReleaseKind {
    Stable,
    PrereleaseFallback,
    YankedFallback,
}

#[derive(Clone, Debug)]
pub struct DefaultReleaseCandidate {
    pub version: ReleaseCandidate,
    pub kind: DefaultReleaseKind,
}

/// Pick the same default from a lightweight projection used by listing pages.
pub fn default_release_candidate(versions: &[ReleaseCandidate]) -> Option<DefaultReleaseCandidate> {
    let highest = |predicate: fn(&ReleaseCandidate, &semver::Version) -> bool| {
        let mut best: Option<(semver::Version, ReleaseCandidate)> = None;
        for version in versions {
            let Ok(parsed) = semver::Version::parse(&version.version) else {
                continue;
            };
            if predicate(version, &parsed)
                && best
                    .as_ref()
                    .is_none_or(|(best_version, _)| parsed > *best_version)
            {
                best = Some((parsed, version.clone()));
            }
        }
        best.map(|(_, version)| version)
    };
    if let Some(version) = highest(|version, parsed| version.yanked == 0 && parsed.pre.is_empty()) {
        return Some(DefaultReleaseCandidate {
            version,
            kind: DefaultReleaseKind::Stable,
        });
    }
    if let Some(version) = highest(|version, _| version.yanked == 0) {
        return Some(DefaultReleaseCandidate {
            version,
            kind: DefaultReleaseKind::PrereleaseFallback,
        });
    }
    highest(|_, _| true).map(|version| DefaultReleaseCandidate {
        version,
        kind: DefaultReleaseKind::YankedFallback,
    })
}

fn release_candidate(row: &sea_orm::QueryResult) -> ReleaseCandidate {
    ReleaseCandidate {
        id: row.try_get("", "id").unwrap_or_default(),
        package_id: row.try_get("", "package_id").unwrap_or_default(),
        version: row.try_get("", "version").unwrap_or_default(),
        size_bytes: row.try_get("", "size_bytes").unwrap_or_default(),
        checksum_sha256: row.try_get("", "checksum_sha256").unwrap_or_default(),
        yanked: row.try_get("", "yanked").unwrap_or_default(),
        sema_version_req: row.try_get("", "sema_version_req").ok(),
        tarball_url: row.try_get("", "tarball_url").ok(),
        published_at: row.try_get("", "published_at").unwrap_or_default(),
    }
}

/// List releases without loading their potentially large rendered documents.
pub async fn list_candidates_for_package<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
) -> Vec<ReleaseCandidate> {
    list_candidates_for_packages(db, &[package_id]).await
}

/// Batch lightweight releases for several package listings in one query.
pub async fn list_candidates_for_packages<C: ConnectionTrait>(
    db: &C,
    package_ids: &[i64],
) -> Vec<ReleaseCandidate> {
    if package_ids.is_empty() {
        return Vec::new();
    }
    let placeholders = std::iter::repeat_n("?", package_ids.len())
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT id, package_id, version, size_bytes, checksum_sha256, yanked, sema_version_req, tarball_url, published_at FROM package_versions WHERE package_id IN ({placeholders})"
    );
    db.query_all(crate::db::stmt(
        db.get_database_backend(),
        &sql,
        package_ids.iter().copied().map(Into::into),
    ))
    .await
    .unwrap_or_default()
    .iter()
    .map(release_candidate)
    .collect()
}

/// Fetch the next bounded batch of upload-backed archive metadata for the
/// one-time release-documentation backfill.
pub async fn document_backfill_targets<C: ConnectionTrait>(
    db: &C,
    after_id: i64,
    limit: i64,
) -> Result<Vec<DocumentBackfillTarget>, DbErr> {
    let rows = db
        .query_all(crate::db::stmt(
            db.get_database_backend(),
            "SELECT id, blob_key, checksum_sha256, tarball_url FROM package_versions WHERE id > ? ORDER BY id ASC LIMIT ?",
            [after_id.into(), limit.into()],
        ))
        .await?;
    Ok(rows
        .iter()
        .map(|row| DocumentBackfillTarget {
            id: row.try_get("", "id").unwrap_or_default(),
            blob_key: row.try_get("", "blob_key").unwrap_or_default(),
            checksum_sha256: row.try_get("", "checksum_sha256").unwrap_or_default(),
            tarball_url: row.try_get("", "tarball_url").ok(),
        })
        .collect())
}

/// Load one release including its stored release documentation.
pub async fn find_by_id<C: ConnectionTrait>(
    db: &C,
    id: i64,
) -> Result<Option<package_version::Model>, DbErr> {
    package_version::Entity::find_by_id(id).one(db).await
}

pub async fn set_documents<C: ConnectionTrait>(
    db: &C,
    version_id: i64,
    docs: ReleaseDocuments,
) -> Result<(), DbErr> {
    package_version::Entity::update_many()
        .col_expr(
            package_version::Column::ReadmeRaw,
            Expr::value(docs.readme_raw),
        )
        .col_expr(
            package_version::Column::ReadmeHtml,
            Expr::value(docs.readme_html),
        )
        .col_expr(
            package_version::Column::ChangelogRaw,
            Expr::value(docs.changelog_raw),
        )
        .col_expr(
            package_version::Column::ChangelogHtml,
            Expr::value(docs.changelog_html),
        )
        .filter(package_version::Column::Id.eq(version_id))
        .exec(db)
        .await
        .map(|_| ())
}

pub fn sort_candidates_for_display(versions: &mut [ReleaseCandidate]) {
    versions.sort_by(|left, right| {
        match (
            semver::Version::parse(&left.version),
            semver::Version::parse(&right.version),
        ) {
            (Ok(left), Ok(right)) => right.cmp(&left),
            (Ok(_), Err(_)) => std::cmp::Ordering::Less,
            (Err(_), Ok(_)) => std::cmp::Ordering::Greater,
            (Err(_), Err(_)) => right.id.cmp(&left.id),
        }
    });
}

/// Where a download should be served from: either an upstream redirect
/// (`tarball_url`) or an on-disk blob (`blob_key`).
pub struct DownloadTarget {
    pub blob_key: String,
    pub tarball_url: Option<String>,
}

/// All versions of a package, newest first.
pub async fn list_for_package<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
) -> Result<Vec<package_version::Model>, DbErr> {
    package_version::Entity::find()
        .filter(package_version::Column::PackageId.eq(package_id))
        .order_by_desc(package_version::Column::PublishedAt)
        .all(db)
        .await
}

/// All versions of a package ordered by insertion id, newest first — used by
/// the package page, where "latest" is the highest-id row.
pub async fn list_for_package_by_id<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
) -> Vec<package_version::Model> {
    package_version::Entity::find()
        .filter(package_version::Column::PackageId.eq(package_id))
        .order_by_desc(package_version::Column::Id)
        .all(db)
        .await
        .unwrap_or_default()
}

/// Whether `(package_id, version)` already exists.
pub async fn exists<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    version: &str,
) -> Result<bool, DbErr> {
    let count = package_version::Entity::find()
        .filter(package_version::Column::PackageId.eq(package_id))
        .filter(package_version::Column::Version.eq(version))
        .count(db)
        .await?;
    Ok(count > 0)
}

/// Insert a new version row, stamping `published_at` in Rust.
#[allow(clippy::too_many_arguments)]
pub async fn create<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    version: &str,
    checksum_sha256: &str,
    blob_key: String,
    size_bytes: i64,
    sema_version_req: Option<String>,
    docs: ReleaseDocuments,
) -> Result<package_version::Model, DbErr> {
    let row = package_version::ActiveModel {
        package_id: Set(package_id),
        version: Set(version.to_string()),
        checksum_sha256: Set(checksum_sha256.to_string()),
        blob_key: Set(blob_key),
        size_bytes: Set(size_bytes),
        sema_version_req: Set(sema_version_req),
        readme_raw: Set(docs.readme_raw),
        readme_html: Set(docs.readme_html),
        changelog_raw: Set(docs.changelog_raw),
        changelog_html: Set(docs.changelog_html),
        published_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await
}

/// Insert a GitHub-synced version: no on-disk blob (an upstream tarball URL is
/// stored instead), zero size, empty checksum. `published_at` stamped in Rust.
pub async fn create_github_version<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    version: &str,
    tarball_url: String,
    sema_version_req: Option<String>,
    docs: ReleaseDocuments,
) -> Result<package_version::Model, DbErr> {
    let row = package_version::ActiveModel {
        package_id: Set(package_id),
        version: Set(version.to_string()),
        checksum_sha256: Set(String::new()),
        blob_key: Set(String::new()),
        size_bytes: Set(0),
        sema_version_req: Set(sema_version_req),
        tarball_url: Set(Some(tarball_url)),
        readme_raw: Set(docs.readme_raw),
        readme_html: Set(docs.readme_html),
        changelog_raw: Set(docs.changelog_raw),
        changelog_html: Set(docs.changelog_html),
        published_at: Set(time::now()),
        ..Default::default()
    };
    row.insert(db).await
}

/// Resolve a downloadable, non-yanked version by package name + version.
pub async fn download_target<C: ConnectionTrait>(
    db: &C,
    name: &str,
    version: &str,
) -> Result<Option<DownloadTarget>, DbErr> {
    let row = db
        .query_one(crate::db::stmt(
            db.get_database_backend(),
            r#"SELECT pv.blob_key, pv.tarball_url FROM package_versions pv
               JOIN packages p ON p.id = pv.package_id
               WHERE p.name = ? AND pv.version = ? AND pv.yanked = 0"#,
            [name.into(), version.into()],
        ))
        .await?;

    Ok(row.map(|r| DownloadTarget {
        blob_key: r.try_get("", "blob_key").unwrap_or_default(),
        tarball_url: r.try_get("", "tarball_url").ok(),
    }))
}

/// Mark `(package_id, version)` yanked; returns the number of rows affected.
pub async fn yank<C: ConnectionTrait>(
    db: &C,
    package_id: i64,
    version: &str,
) -> Result<u64, DbErr> {
    let result = package_version::Entity::update_many()
        .col_expr(package_version::Column::Yanked, Expr::value(1))
        .filter(package_version::Column::PackageId.eq(package_id))
        .filter(package_version::Column::Version.eq(version))
        .exec(db)
        .await?;
    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: i64, version: &str, yanked: bool) -> ReleaseCandidate {
        ReleaseCandidate {
            id,
            package_id: 1,
            version: version.to_string(),
            size_bytes: 0,
            checksum_sha256: String::new(),
            yanked: i32::from(yanked),
            sema_version_req: None,
            tarball_url: None,
            published_at: String::new(),
        }
    }

    #[test]
    fn default_candidate_uses_highest_non_yanked_stable_semver() {
        let releases = [
            candidate(99, "1.5.0", false),
            candidate(1, "2.0.0", false),
            candidate(100, "3.0.0-rc.1", false),
            candidate(101, "9.0.0", true),
        ];

        let selected = default_release_candidate(&releases).unwrap();
        assert_eq!(selected.version.version, "2.0.0");
        assert_eq!(selected.kind, DefaultReleaseKind::Stable);
    }

    #[test]
    fn default_candidate_falls_back_to_non_yanked_prerelease() {
        let releases = [
            candidate(1, "1.0.0-alpha.1", false),
            candidate(2, "1.0.0-rc.2", false),
            candidate(3, "2.0.0", true),
        ];

        let selected = default_release_candidate(&releases).unwrap();
        assert_eq!(selected.version.version, "1.0.0-rc.2");
        assert_eq!(selected.kind, DefaultReleaseKind::PrereleaseFallback);
    }

    #[test]
    fn default_candidate_marks_all_yanked_releases() {
        let releases = [candidate(1, "1.0.0", true), candidate(2, "2.0.0", true)];

        let selected = default_release_candidate(&releases).unwrap();
        assert_eq!(selected.version.version, "2.0.0");
        assert_eq!(selected.kind, DefaultReleaseKind::YankedFallback);
    }
}
