pub mod api;
pub mod audit;
pub mod auth;
pub mod blob;
pub mod cli;
pub mod config;
pub mod crypto;
pub mod dal;
pub mod db;
pub mod entity;
pub mod github;
pub mod github_sync;
pub mod health;
pub mod migration;
pub mod ratelimit;
pub mod tarball;
pub mod telemetry;
pub mod web;

use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post, put},
    Router,
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

/// Renders the current Prometheus metrics as an exposition-format string. An
/// `Arc<dyn Fn>` so the type is observability-feature-agnostic (the concrete
/// Prometheus handle lives behind the `observability` feature).
pub type MetricsRender = Arc<dyn Fn() -> String + Send + Sync>;

pub struct AppState {
    pub db: db::Db,
    pub config: config::Config,
    pub blobs: blob::BlobStore,
    /// `Some` when Prometheus metrics are enabled; serves `/metrics`.
    pub metrics_render: Option<MetricsRender>,
    /// TTL cache for the admin dashboard summary.
    pub stats_cache: dal::admin::StatsCache,
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let config = &state.config;

    // Health probes, web pages, static assets, and OAuth redirects — never rate
    // limited (probes and asset fetches would otherwise trip the limiter).
    let mut public = Router::new()
        .route("/healthz", get(health::liveness))
        .route("/readyz", get(health::readiness))
        // Web pages
        .route("/", get(web::index))
        .route("/search", get(web::search))
        .route("/packages/{name}", get(web::package_detail))
        .route("/login", get(web::login))
        .route("/account", get(web::account))
        .route("/link", get(web::link_page))
        .route("/admin", get(web::admin_page))
        // GitHub OAuth
        .route("/auth/github", get(github::start))
        .route("/auth/github/callback", get(github::callback))
        // Static files
        .nest_service("/static", ServeDir::new("static"));

    // Prometheus scrape endpoint, only when metrics are enabled. Unlimited and
    // unauthenticated (scrape it on a private network / behind the proxy).
    if state.metrics_render.is_some() {
        public = public.route("/metrics", get(metrics_endpoint));
    }

    // Auth endpoints — stricter, fixed rate limit (credential brute-forcing).
    let auth = Router::new()
        .route("/api/v1/auth/register", post(api::auth::register))
        .route("/api/v1/auth/login", post(api::auth::login));
    let auth = ratelimit::auth(auth, config);

    // Install hot path — package metadata + tarball download. Read-only and
    // pulled in bulk during dependency resolution, so it gets its own generous
    // limiter tier rather than sharing the strict global one (which would 429
    // legitimate multi-package installs).
    let read = Router::new()
        .route("/api/v1/packages/{name}", get(api::packages::get_package))
        .route(
            "/api/v1/packages/{name}/downloads",
            get(api::packages::download_stats),
        )
        .route(
            "/api/v1/packages/{name}/{version}/download",
            get(api::packages::download),
        );
    let read = ratelimit::read(read, config);

    // The rest of the API — global rate limit.
    let api = Router::new()
        .route("/api/v1/auth/logout", post(api::auth::logout))
        // Account
        .route("/api/v1/account", put(api::account::update))
        // Tokens API
        .route(
            "/api/v1/tokens",
            post(api::tokens::create).get(api::tokens::list),
        )
        .route("/api/v1/tokens/{id}", delete(api::tokens::revoke))
        // Packages API
        .route(
            "/api/v1/packages/{name}/{version}",
            // Axum's default extractor body cap (2 MB) would silently override
            // max_tarball_bytes; allow the configured size plus multipart framing.
            put(api::packages::publish).layer(DefaultBodyLimit::max(
                config.max_tarball_bytes + 1024 * 1024,
            )),
        )
        .route(
            "/api/v1/packages/{name}/{version}/yank",
            post(api::packages::yank),
        )
        .route(
            "/api/v1/packages/{name}/owners",
            get(api::packages::list_owners)
                .put(api::packages::add_owner)
                .delete(api::packages::remove_owner),
        )
        .route("/api/v1/search", get(api::packages::search))
        // GitHub-linked packages API
        .route("/api/v1/packages/link", post(api::github::link))
        .route("/api/v1/packages/{name}/sync", post(api::github::sync))
        .route("/api/v1/webhooks/github", post(api::github::webhook))
        // Admin API
        .route("/api/v1/admin/stats", get(api::admin::stats))
        .route("/api/v1/admin/users", get(api::admin::list_users))
        .route("/api/v1/admin/users/{id}", get(api::admin::get_user))
        .route("/api/v1/admin/users/{id}/ban", post(api::admin::ban_user))
        .route(
            "/api/v1/admin/users/{id}/unban",
            post(api::admin::unban_user),
        )
        .route(
            "/api/v1/admin/users/{id}/revoke-tokens",
            post(api::admin::revoke_user_tokens),
        )
        .route(
            "/api/v1/admin/users/{id}/role",
            put(api::admin::set_user_role),
        )
        .route("/api/v1/admin/packages", get(api::admin::list_packages))
        .route(
            "/api/v1/admin/packages/{name}",
            get(api::admin::get_package).delete(api::admin::remove_package),
        )
        .route(
            "/api/v1/admin/packages/{name}/yank-all",
            post(api::admin::yank_all_versions),
        )
        .route(
            "/api/v1/admin/packages/{name}/transfer",
            post(api::admin::transfer_ownership),
        )
        .route("/api/v1/admin/audit", get(api::admin::list_audit))
        .route("/api/v1/admin/reports", get(api::admin::list_reports))
        .route(
            "/api/v1/admin/reports/{id}/action",
            post(api::admin::action_report),
        )
        .route(
            "/api/v1/admin/reports/{id}/dismiss",
            post(api::admin::dismiss_report),
        )
        // Report submission (any authenticated user)
        .route("/api/v1/reports", post(api::admin::submit_report));
    let api = ratelimit::global(api, config);

    let app = public
        .merge(auth)
        .merge(read)
        .merge(api)
        // Prometheus RED metrics per matched route (runs inside routing, so the
        // route template is available for a bounded-cardinality label). A no-op
        // until a metrics recorder is installed.
        .route_layer(axum::middleware::from_fn(telemetry::track_metrics))
        // Runs outside the per-tier governor layers so it can see (and fix up)
        // the 429s they emit: guarantees an actionable `Retry-After`.
        .layer(axum::middleware::from_fn(ratelimit::ensure_retry_after));

    // Outermost: a span per request (method, path, status, latency) that parents
    // the handler/DAL spans. A no-op without a tracing subscriber; exported to
    // OpenTelemetry when traces are configured.
    app.layer(TraceLayer::new_for_http()).with_state(state)
}

/// Serve the current Prometheus metrics. Only routed when metrics are enabled,
/// so `metrics_render` is always `Some` here.
async fn metrics_endpoint(
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    match &state.metrics_render {
        Some(render) => render().into_response(),
        None => axum::http::StatusCode::NOT_FOUND.into_response(),
    }
}
