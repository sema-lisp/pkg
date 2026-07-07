//! IP-keyed request rate limiting (GCRA, via `tower_governor`).
//!
//! Two tiers, applied to disjoint route groups in [`crate::build_router`]:
//!
//! - **Global** — guards the general API surface (downloads, search, publish,
//!   admin). Tunable via `RATE_LIMIT_RPS` / `RATE_LIMIT_BURST`.
//! - **Auth** — a stricter, fixed limit on `register`/`login` to blunt
//!   credential brute-forcing.
//!
//! Health probes, static assets, and web pages are intentionally *not* limited.
//!
//! The key is the client IP via [`SmartIpKeyExtractor`], which honours
//! `X-Forwarded-For` / `X-Real-IP` / `Forwarded` before falling back to the peer
//! address — so it stays correct behind a reverse proxy. This requires the
//! server to be served with `into_make_service_with_connect_info::<SocketAddr>()`
//! (see `main.rs`) so a peer address exists when no forwarded header is present.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use tower_governor::{
    governor::GovernorConfigBuilder, key_extractor::SmartIpKeyExtractor, GovernorLayer,
};

use crate::config::Config;

/// Layer one limiter tier onto `router` and spawn its background eviction task.
/// The GCRA state map grows with distinct client IPs; `retain_recent` prunes
/// idle entries so memory stays bounded on a long-running server.
///
/// The concrete config type is left to inference — `use_headers()` swaps the
/// middleware type parameter, so naming it here would be brittle.
fn apply<S>(router: Router<S>, per_second: u32, burst: u32) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    let conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(per_second.max(1) as u64)
            .burst_size(burst.max(1))
            .key_extractor(SmartIpKeyExtractor)
            .use_headers() // emit x-ratelimit-* + retry-after on 429
            .finish()
            .expect("valid rate-limit config"),
    );

    let limiter = conf.limiter().clone();
    tokio::spawn(async move {
        let interval = Duration::from_secs(60);
        loop {
            tokio::time::sleep(interval).await;
            limiter.retain_recent();
        }
    });

    router.layer(GovernorLayer::new(conf))
}

/// Apply the **global** API rate limit to `router`, unless disabled in config.
pub fn global<S>(router: Router<S>, config: &Config) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.rate_limit_enabled {
        return router;
    }
    apply(router, config.rate_limit_rps, config.rate_limit_burst)
}

/// Apply the stricter **auth** rate limit to `router`, unless disabled in config.
/// Fixed (not env-tunable): 5-request burst, replenishing 1/sec per IP.
pub fn auth<S>(router: Router<S>, config: &Config) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.rate_limit_enabled {
        return router;
    }
    apply(router, 1, 5)
}
