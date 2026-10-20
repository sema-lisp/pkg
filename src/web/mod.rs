use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{auth::get_session_user, dal, AppState};

pub mod feeds;
pub mod sitemap;

fn render<T: Template>(tmpl: T) -> impl IntoResponse {
    match tmpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Template error: {e}"),
        )
            .into_response(),
    }
}

/// Build an XML response with an explicit content type and cache policy. Mirrors
/// the header-tuple pattern used for other non-HTML responses (cf. the tarball
/// download handler) since the `render` helper only produces `text/html`.
pub(crate) fn respond_xml(body: String, cache_control: &'static str) -> axum::response::Response {
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CACHE_CONTROL, cache_control),
        ],
        body,
    )
        .into_response()
}

#[derive(Template)]
#[template(path = "404.html")]
pub struct NotFoundTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub heading: String,
    pub message: String,
    pub subject: Option<String>,
}

/// Render the styled 404 page, carrying the current session's header state.
async fn render_not_found(
    state: &AppState,
    headers: &axum::http::HeaderMap,
    heading: &str,
    message: &str,
    subject: Option<String>,
) -> axum::response::Response {
    let si = get_session_info(state, headers).await;
    (
        StatusCode::NOT_FOUND,
        render(NotFoundTemplate {
            username: si.username,
            is_admin: si.is_admin,
            base_url: state.config.site_url().to_string(),
            heading: heading.to_string(),
            message: message.to_string(),
            subject,
        }),
    )
        .into_response()
}

/// Router fallback for unmatched routes: JSON for the API surface, the HTML
/// not-found page for everything else.
pub async fn fallback(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    uri: axum::http::Uri,
) -> axum::response::Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({ "error": "Not found" })),
        )
            .into_response();
    }
    render_not_found(
        &state,
        &headers,
        "Page not found",
        "We couldn't find the page you're looking for",
        None,
    )
    .await
}

struct SessionInfo {
    username: Option<String>,
    is_admin: bool,
}

async fn get_session_info(state: &AppState, headers: &axum::http::HeaderMap) -> SessionInfo {
    let user = async {
        let cookie = headers.get(header::COOKIE)?.to_str().ok()?;
        let session_id = cookie
            .split(';')
            .filter_map(|c| c.trim().strip_prefix("session="))
            .next()?;
        get_session_user(&state.db, session_id).await
    }
    .await;

    match user {
        Some(u) => SessionInfo {
            username: Some(u.username),
            is_admin: u.is_admin,
        },
        None => SessionInfo {
            username: None,
            is_admin: false,
        },
    }
}

// ── Data types for templates ──

pub struct PackageSummary {
    pub name: String,
    pub description: String,
    pub latest_version: String,
    pub published_at: String,
}

#[derive(Clone)]
pub struct VersionInfo {
    pub version: String,
    pub published_at: String,
    pub yanked: bool,
    pub size_bytes: i64,
    pub checksum_sha256: String,
    pub sema_version_req: Option<String>,
}

pub struct DepInfo {
    pub dependency_name: String,
    pub version_req: String,
}

pub struct OwnerInfo {
    pub name: String,
    /// True for the official house account(s) — renders a verified badge.
    pub official: bool,
}

pub struct TokenInfo {
    pub id: i64,
    pub name: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

// ── Templates ──

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub total_packages: i64,
    pub recent: Vec<PackageSummary>,
}

#[derive(Template)]
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub query: String,
    pub packages: Vec<PackageSummary>,
    pub total: i64,
    pub page: i64,
    pub per_page: i64,
}

#[derive(Template)]
#[template(path = "package.html")]
pub struct PackageTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub name: String,
    pub description: String,
    pub repository_url: Option<String>,
    pub source: String,
    pub github_repo: Option<String>,
    pub owners: Vec<OwnerInfo>,
    pub versions: Vec<VersionInfo>,
    pub selected_version: Option<VersionInfo>,
    pub release_notice: Option<String>,
    pub installable: bool,
    pub deps: Vec<DepInfo>,
    pub total_downloads: i64,
    pub readme_html: Option<String>,
    pub changelog_html: Option<String>,
    /// Pre-serialized schema.org JSON-LD for the package (SoftwareSourceCode).
    pub jsonld: Option<String>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub github_enabled: bool,
}

#[derive(Template)]
#[template(path = "link.html")]
pub struct LinkTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub github_connected: bool,
    pub github_login: Option<String>,
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
    pub user_email: String,
    pub user_homepage: Option<String>,
    pub github_connected: bool,
    pub github_login: Option<String>,
    pub packages: Vec<PackageSummary>,
    pub tokens: Vec<TokenInfo>,
}

#[derive(Template)]
#[template(path = "admin.html")]
pub struct AdminTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub base_url: String,
}

// ── Handlers ──

pub async fn admin_page(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let si = get_session_info(&state, &headers).await;
    match si.username {
        None => Redirect::to("/login").into_response(),
        Some(ref _u) if !si.is_admin => {
            (StatusCode::FORBIDDEN, "Admin access required").into_response()
        }
        Some(_) => render(AdminTemplate {
            username: si.username,
            is_admin: si.is_admin,
            base_url: state.config.site_url().to_string(),
        })
        .into_response(),
    }
}

pub async fn index(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let si = get_session_info(&state, &headers).await;

    let total_packages = dal::packages::count(&state.db).await;

    let recent = dal::packages::recent(&state.db, 10)
        .await
        .into_iter()
        .map(
            |(_, name, description, latest_version, published_at)| PackageSummary {
                name,
                description,
                latest_version,
                published_at,
            },
        )
        .collect();

    render(IndexTemplate {
        username: si.username,
        is_admin: si.is_admin,
        base_url: state.config.site_url().to_string(),
        total_packages,
        recent,
    })
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: Option<String>,
    pub page: Option<i64>,
}

pub async fn search(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    Query(params): Query<SearchParams>,
) -> impl IntoResponse {
    let si = get_session_info(&state, &headers).await;
    let query = params.q.unwrap_or_default();
    let page = params.page.unwrap_or(1).max(1);
    let per_page: i64 = 20;
    let offset = (page - 1) * per_page;

    let packages = dal::packages::search_page(&state.db, &query, per_page, offset)
        .await
        .into_iter()
        .map(
            |(_, name, description, latest_version, published_at)| PackageSummary {
                name,
                description,
                latest_version,
                published_at,
            },
        )
        .collect();

    let total = dal::packages::search_count(&state.db, &query)
        .await
        .unwrap_or(0);

    render(SearchTemplate {
        username: si.username,
        is_admin: si.is_admin,
        base_url: state.config.site_url().to_string(),
        query,
        packages,
        total,
        page,
        per_page,
    })
}

pub async fn package_detail(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path(name): axum::extract::Path<String>,
) -> axum::response::Response {
    package_page(&state, &headers, name, None).await
}

pub async fn package_version_detail(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
    axum::extract::Path((name, version)): axum::extract::Path<(String, String)>,
) -> axum::response::Response {
    package_page(&state, &headers, name, Some(version)).await
}

async fn package_page(
    state: &Arc<AppState>,
    headers: &axum::http::HeaderMap,
    name: String,
    requested_version: Option<String>,
) -> axum::response::Response {
    let si = get_session_info(state, headers).await;

    let pkg = dal::packages::find_by_name(&state.db, &name)
        .await
        .ok()
        .flatten();

    let pkg = match pkg {
        Some(p) => p,
        None => {
            return render_not_found(
                state,
                headers,
                "Package not found",
                "No package is registered under the name",
                Some(name),
            )
            .await
        }
    };

    let pkg_id = pkg.id;
    let description = pkg.description.clone();
    let repository_url = pkg.repository_url.clone();
    let source = pkg.source.clone();
    let github_repo = pkg.github_repo.clone();
    let mut version_models = dal::versions::list_candidates_for_package(&state.db, pkg_id).await;
    dal::versions::sort_candidates_for_display(&mut version_models);

    let selected = match requested_version {
        Some(version) => match version_models
            .iter()
            .find(|release| release.version == version)
        {
            Some(release) => Some((release.clone(), None)),
            None => {
                return render_not_found(
                    state,
                    headers,
                    "Release not found",
                    "No release is registered under this version",
                    Some(format!("{name}@{version}")),
                )
                .await
            }
        },
        None => dal::versions::default_release_candidate(&version_models).map(|release| {
            let notice = match release.kind {
                dal::versions::DefaultReleaseKind::Stable => None,
                dal::versions::DefaultReleaseKind::PrereleaseFallback => Some(
                    "No stable release is available. Showing the highest installable prerelease."
                        .to_string(),
                ),
                dal::versions::DefaultReleaseKind::YankedFallback => Some(
                    "All published releases are yanked. This page is shown for reference only."
                        .to_string(),
                ),
            };
            (release.version, notice)
        }),
    };

    let (selected_model, release_notice) = match selected {
        Some((release, notice)) => (Some(release), notice),
        None if version_models.is_empty() => (
            None,
            Some("No published release is available yet.".to_string()),
        ),
        None => (
            None,
            Some("No usable semantic-version release is available.".to_string()),
        ),
    };
    let installable = selected_model
        .as_ref()
        .is_some_and(|release| release.yanked == 0);
    let release_notice = release_notice.or_else(|| {
        selected_model
            .as_ref()
            .filter(|release| release.yanked != 0)
            .map(|_| "This release is yanked and cannot be installed.".to_string())
    });
    let selected_version = selected_model.as_ref().map(candidate_info);
    let docs = match selected_model.as_ref() {
        Some(release) => dal::versions::find_by_id(&state.db, release.id)
            .await
            .ok()
            .flatten(),
        None => None,
    };
    let readme_html = docs
        .as_ref()
        .and_then(|release| release.readme_html.clone());
    let changelog_html = docs
        .as_ref()
        .and_then(|release| release.changelog_html.clone());

    let versions: Vec<VersionInfo> = version_models.iter().map(candidate_info).collect();

    let deps = if let Some(release) = selected_model.as_ref() {
        dal::deps::list_for_version(&state.db, release.id)
            .await
            .iter()
            .map(|d| DepInfo {
                dependency_name: d.dependency_name.clone(),
                version_req: d.version_req.clone(),
            })
            .collect()
    } else {
        vec![]
    };

    let owners: Vec<OwnerInfo> = dal::owners::list_usernames(&state.db, pkg_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|name| OwnerInfo {
            official: crate::auth::is_official(&name),
            name,
        })
        .collect();

    let total_downloads = dal::downloads::total(&state.db, &name).await.unwrap_or(0);

    // schema.org JSON-LD for richer search results. Serialized with serde_json
    // (handles escaping); `</` is further escaped so the payload can't break out
    // of the surrounding <script> element.
    let jsonld = {
        let site = state.config.site_url();
        let mut obj = serde_json::json!({
            "@context": "https://schema.org",
            "@type": "SoftwareSourceCode",
            "name": &name,
            "description": &description,
            "url": match selected_model.as_ref() {
                Some(release) => format!("{site}/packages/{name}/{}", release.version),
                None => format!("{site}/packages/{name}"),
            },
            "programmingLanguage": "Sema",
        });
        if let Some(repo) = &repository_url {
            obj["codeRepository"] = serde_json::Value::String(repo.clone());
        }
        serde_json::to_string(&obj)
            .ok()
            .map(|s| s.replace("</", "<\\/"))
    };

    render(PackageTemplate {
        username: si.username,
        is_admin: si.is_admin,
        base_url: state.config.site_url().to_string(),
        name,
        description,
        repository_url,
        source,
        github_repo,
        owners,
        versions,
        selected_version,
        release_notice,
        installable,
        deps,
        total_downloads,
        readme_html,
        changelog_html,
        jsonld,
    })
    .into_response()
}

fn candidate_info(version: &dal::versions::ReleaseCandidate) -> VersionInfo {
    VersionInfo {
        version: version.version.clone(),
        published_at: version.published_at.clone(),
        yanked: version.yanked != 0,
        size_bytes: version.size_bytes,
        checksum_sha256: version.checksum_sha256.clone(),
        sema_version_req: version.sema_version_req.clone(),
    }
}

pub async fn login(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let si = get_session_info(&state, &headers).await;
    if si.username.is_some() {
        return Redirect::to("/account").into_response();
    }
    let github_enabled = state.config.github_client_id.is_some();
    render(LoginTemplate {
        username: None,
        is_admin: false,
        base_url: state.config.site_url().to_string(),
        github_enabled,
    })
    .into_response()
}

pub async fn link_page(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let session_id = cookie
        .split(';')
        .filter_map(|c| c.trim().strip_prefix("session="))
        .next();

    let session_id = match session_id {
        Some(s) => s,
        None => return Redirect::to("/login").into_response(),
    };

    let user = match get_session_user(&state.db, session_id).await {
        Some(u) => u,
        None => return Redirect::to("/login").into_response(),
    };

    let github_row = dal::oauth::find_active(&state.db, user.id)
        .await
        .ok()
        .flatten();

    let github_connected = github_row.is_some();
    let github_login = github_row.and_then(|r| r.provider_login);

    render(LinkTemplate {
        username: Some(user.username),
        is_admin: user.is_admin,
        base_url: state.config.site_url().to_string(),
        github_connected,
        github_login,
    })
    .into_response()
}

pub async fn account(
    State(state): State<Arc<AppState>>,
    headers: axum::http::HeaderMap,
) -> impl IntoResponse {
    let cookie = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let session_id = cookie
        .split(';')
        .filter_map(|c| c.trim().strip_prefix("session="))
        .next();

    let session_id = match session_id {
        Some(s) => s,
        None => return Redirect::to("/login").into_response(),
    };

    let user = match get_session_user(&state.db, session_id).await {
        Some(u) => u,
        None => return Redirect::to("/login").into_response(),
    };

    // Get user details
    let user_model = dal::users::find_by_id(&state.db, user.id)
        .await
        .ok()
        .flatten();

    let (user_email, user_homepage) = match user_model {
        Some(u) => (u.email, u.homepage),
        None => (String::new(), None),
    };

    // Get user's packages (JOIN query)
    let packages = dal::packages::list_for_owner(&state.db, user.id)
        .await
        .into_iter()
        .map(
            |(_, name, description, latest_version, published_at)| PackageSummary {
                name,
                description,
                latest_version,
                published_at,
            },
        )
        .collect();

    // Get tokens
    let tokens = dal::tokens::list_active_for_user(&state.db, user.id)
        .await
        .iter()
        .map(|t| TokenInfo {
            id: t.id,
            name: t.name.clone(),
            created_at: t.created_at.clone(),
            last_used_at: t.last_used_at.clone(),
        })
        .collect();

    // Get GitHub connection status
    let github_row = dal::oauth::find_active(&state.db, user.id)
        .await
        .ok()
        .flatten();

    let github_connected = github_row.is_some();
    let github_login = github_row.and_then(|r| r.provider_login);

    render(AccountTemplate {
        username: Some(user.username.clone()),
        is_admin: user.is_admin,
        base_url: state.config.site_url().to_string(),
        user_email,
        user_homepage,
        github_connected,
        github_login,
        packages,
        tokens,
    })
    .into_response()
}
