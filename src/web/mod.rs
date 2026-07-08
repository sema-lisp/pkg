use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Redirect},
};
use serde::Deserialize;
use std::sync::Arc;

use crate::{auth::get_session_user, dal, AppState};

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

#[derive(Template)]
#[template(path = "404.html")]
pub struct NotFoundTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
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
    pub total_packages: i64,
    pub recent: Vec<PackageSummary>,
}

#[derive(Template)]
#[template(path = "search.html")]
pub struct SearchTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
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
    pub name: String,
    pub description: String,
    pub repository_url: Option<String>,
    pub source: String,
    pub github_repo: Option<String>,
    pub owners: Vec<OwnerInfo>,
    pub versions: Vec<VersionInfo>,
    pub deps: Vec<DepInfo>,
    pub total_downloads: i64,
    pub readme_html: Option<String>,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub github_enabled: bool,
}

#[derive(Template)]
#[template(path = "link.html")]
pub struct LinkTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
    pub github_connected: bool,
    pub github_login: Option<String>,
}

#[derive(Template)]
#[template(path = "account.html")]
pub struct AccountTemplate {
    pub username: Option<String>,
    pub is_admin: bool,
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
            |(name, description, latest_version, published_at)| PackageSummary {
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
            |(name, description, latest_version, published_at)| PackageSummary {
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
) -> impl IntoResponse {
    let si = get_session_info(&state, &headers).await;

    let pkg = dal::packages::find_by_name(&state.db, &name)
        .await
        .ok()
        .flatten();

    let pkg = match pkg {
        Some(p) => p,
        None => {
            return render_not_found(
                &state,
                &headers,
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
    let readme_html = pkg.readme_html.clone();

    let version_models = dal::versions::list_for_package_by_id(&state.db, pkg_id).await;

    let versions: Vec<VersionInfo> = version_models
        .iter()
        .map(|v| VersionInfo {
            version: v.version.clone(),
            published_at: v.published_at.clone(),
            yanked: v.yanked != 0,
            size_bytes: v.size_bytes,
            checksum_sha256: v.checksum_sha256.clone(),
            sema_version_req: v.sema_version_req.clone(),
        })
        .collect();

    // Get deps for latest version
    let deps = if let Some(latest) = version_models.first() {
        dal::deps::list_for_version(&state.db, latest.id)
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

    render(PackageTemplate {
        username: si.username,
        is_admin: si.is_admin,
        name,
        description,
        repository_url,
        source,
        github_repo,
        owners,
        versions,
        deps,
        total_downloads,
        readme_html,
    })
    .into_response()
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
            |(name, description, latest_version, published_at)| PackageSummary {
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
        user_email,
        user_homepage,
        github_connected,
        github_login,
        packages,
        tokens,
    })
    .into_response()
}
