//! IP-keyed request rate limiting (GCRA, via `tower_governor`).
//!
//! Three tiers, applied to disjoint route groups in [`crate::build_router`]:
//!
//! - **Read** — the install hot path (package metadata + tarball download).
//!   Deliberately generous, since resolving one project pulls many packages in
//!   a burst from a single IP. Tunable via `RATE_LIMIT_READ_RPS` /
//!   `RATE_LIMIT_READ_BURST`.
//! - **Global** — guards the general/write API surface (publish, owners,
//!   search, tokens, admin). Tunable via `RATE_LIMIT_RPS` / `RATE_LIMIT_BURST`.
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

use axum::{
    extract::Request,
    http::{header::RETRY_AFTER, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
    Router,
};
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
            .per_millisecond(replenish_interval_ms(per_second))
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

/// Apply the generous **read/install** rate limit to `router`, unless disabled
/// in config. Covers the endpoints an install pulls in bulk (package metadata
/// and tarball downloads), so a normal multi-package install is never throttled
/// while a runaway client loop is still bounded.
pub fn read<S>(router: Router<S>, config: &Config) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    if !config.rate_limit_enabled {
        return router;
    }
    apply(
        router,
        config.rate_limit_read_rps,
        config.rate_limit_read_burst,
    )
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

/// Ensure every `429 Too Many Requests` carries an actionable `Retry-After`.
///
/// `tower_governor`'s `use_headers()` emits `Retry-After` in whole seconds, so a
/// sub-second replenish interval (any tier above 1 rps) rounds down to
/// `Retry-After: 0` — which tells a client to retry *immediately* and get
/// throttled again. Floor it at 1 second so a compliant client backs off
/// meaningfully. Values of 1s or more (e.g. the auth tier) are left untouched.
pub async fn ensure_retry_after(req: Request, next: Next) -> Response {
    let mut resp = next.run(req).await;
    if resp.status() == StatusCode::TOO_MANY_REQUESTS {
        let needs_floor = resp
            .headers()
            .get(RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .is_none_or(|secs| secs < 1);
        if needs_floor {
            resp.headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from_static("1"));
        }
    }
    resp
}

/// GCRA replenish interval for a sustained requests-per-second rate.
/// tower_governor's `per_second(n)` sets the *interval* (one request per `n`
/// seconds), not the rate — a config of 20 rps would otherwise throttle to one
/// request per 20 seconds once the burst is spent. Clamped to ≥1ms (rates
/// above 1000 rps saturate).
fn replenish_interval_ms(per_second: u32) -> u64 {
    (1000 / per_second.max(1) as u64).max(1)
}

#[cfg(test)]
mod tests {
    use super::replenish_interval_ms;

    #[test]
    fn interval_is_inverse_of_rate() {
        assert_eq!(replenish_interval_ms(20), 50); // 20 rps → every 50ms
        assert_eq!(replenish_interval_ms(1), 1000); // 1 rps → every second
        assert_eq!(replenish_interval_ms(1000), 1);
        assert_eq!(replenish_interval_ms(0), 1000); // clamped like rps=1
        assert_eq!(replenish_interval_ms(5000), 1); // saturates at 1ms
    }
}
