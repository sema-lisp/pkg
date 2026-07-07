//! Liveness and readiness probes.
//!
//! Split along the Kubernetes convention so orchestrators and load balancers can
//! ask two different questions:
//!
//! - **Liveness** (`/healthz`) — "is the process up?" Cheap, dependency-free. A
//!   failing liveness probe means *restart me*, so it must not depend on the DB:
//!   a transient DB outage should not trigger a restart loop.
//! - **Readiness** (`/readyz`) — "should I receive traffic?" Pings the database.
//!   A failing readiness probe means *stop routing to me* until the dependency
//!   recovers, without killing the process.

use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::AppState;

/// Liveness: the process is running and the event loop is responsive.
pub async fn liveness() -> impl IntoResponse {
    (StatusCode::OK, "ok")
}

/// Readiness: the process can serve requests — i.e. the database is reachable.
/// Returns 503 (not 500) so a load balancer drains traffic without a restart.
pub async fn readiness(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    // A trivial round-trip proves the connection pool can reach the DB.
    match state.db.ping().await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ready" })),
        ),
        Err(e) => {
            tracing::warn!("readiness check failed: database unreachable: {e}");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "status": "unavailable",
                    "error": "database unreachable"
                })),
            )
        }
    }
}
