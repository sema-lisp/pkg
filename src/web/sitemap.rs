//! `robots.txt`, the sitemap index, and the chunked child sitemaps.
//!
//! At 10M packages the index must never scan the table: it is sized from
//! `MAX(id)` (an O(1) primary-key probe) and each child is a pure PK **range
//! scan** of at most [`CHUNK`] ids. Because children are keyed by id range, a
//! given `/sitemap/{n}.xml` always covers the same id window — stable URLs,
//! crawler-cache friendly — and each yields ≤ [`CHUNK`] URLs by construction, so
//! the 50,000-URL sitemap-protocol cap can never be exceeded.

use std::sync::Arc;
use std::time::{Duration, Instant};

use askama::Template;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use tokio::sync::Mutex;

use crate::syndication::{url, xml};
use crate::{dal, AppState};

/// Ids per child sitemap. `<= 50_000` keeps each child under the protocol's
/// 50k-URL cap; at ~180 bytes/URL a full child is ~9 MB, well under the 50 MB
/// file cap. 10M ids ⇒ 200 children (ceiling: 50k children × 50k = 2.5B pkgs).
const CHUNK: i64 = 50_000;
/// How long the rendered sitemap index is reused before recompute.
const INDEX_TTL: Duration = Duration::from_secs(300);
const XML_CACHE: &str = "public, max-age=3600";

#[derive(Template)]
#[template(path = "sitemap_index.xml", escape = "none")]
struct SitemapIndexTemplate {
    child_locs: Vec<String>,
}

struct SitemapUrl {
    loc: String,
    lastmod: String,
}

#[derive(Template)]
#[template(path = "sitemap_urlset.xml", escape = "none")]
struct SitemapUrlsetTemplate {
    urls: Vec<SitemapUrl>,
}

/// Render the sitemap index for a given `max_id`: one `<sitemap>` per child.
fn render_index(state: &AppState, max_id: i64) -> String {
    let base = state.config.site_url();
    let num_children = ((max_id + CHUNK - 1) / CHUNK).max(1);
    let child_locs = (1..=num_children)
        .map(|n| xml::escape(&format!("{base}/sitemap/{n}.xml")))
        .collect();
    SitemapIndexTemplate { child_locs }
        .render()
        .unwrap_or_default()
}

/// Cached index snapshot: `(computed_at, max_id, rendered body)`.
type IndexSnapshot = (Instant, i64, Arc<String>);

/// Single-flight TTL cache for the index: stores `(max_id, rendered body)` so
/// concurrent crawlers share one `MAX(id)` probe and one render, and the ETag
/// (derived from `max_id`) is available without re-querying.
#[derive(Clone, Default)]
pub struct SitemapCache {
    inner: Arc<Mutex<Option<IndexSnapshot>>>,
}

impl SitemapCache {
    async fn get(&self, state: &AppState) -> (i64, Arc<String>) {
        let mut guard = self.inner.lock().await;
        if let Some((at, max_id, body)) = guard.as_ref() {
            if at.elapsed() < INDEX_TTL {
                return (*max_id, body.clone());
            }
        }
        let max_id = dal::packages::max_id(&state.db).await;
        let body = Arc::new(render_index(state, max_id));
        *guard = Some((Instant::now(), max_id, body.clone()));
        (max_id, body)
    }
}

pub async fn sitemap_index(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let (max_id, body) = state.sitemap_cache.get(&state).await;
    // The tag changes exactly when the table gains a chunk, so unchanged indexes
    // 304 cheaply for crawlers.
    let etag = format!("W/\"sm-{max_id}\"");
    let matches = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|inm| inm.split(',').any(|t| t.trim() == etag));
    if matches {
        return (
            StatusCode::NOT_MODIFIED,
            [
                (header::ETAG, etag),
                (header::CACHE_CONTROL, XML_CACHE.into()),
            ],
        )
            .into_response();
    }
    (
        [
            (
                header::CONTENT_TYPE,
                "application/xml; charset=utf-8".to_string(),
            ),
            (header::CACHE_CONTROL, XML_CACHE.to_string()),
            (header::ETAG, etag),
        ],
        (*body).clone(),
    )
        .into_response()
}

pub async fn sitemap_chunk(
    State(state): State<Arc<AppState>>,
    Path(chunk): Path<String>,
) -> Response {
    // Crawler-facing URLs are `/sitemap/{n}.xml`; axum can't route a literal
    // suffix on a param segment, so strip `.xml` here.
    let n = match chunk.strip_suffix(".xml").unwrap_or(&chunk).parse::<i64>() {
        Ok(n) if n >= 1 => n,
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    let lo = (n - 1) * CHUNK;
    let hi = n * CHUNK;
    let base = state.config.site_url();
    let urls = dal::packages::id_chunk(&state.db, lo, hi)
        .await
        .into_iter()
        .map(|(_, name, created_at)| {
            let date = created_at.get(..10).unwrap_or(created_at.as_str());
            SitemapUrl {
                loc: xml::escape(&format!("{base}/packages/{}", url::seg(&name))),
                lastmod: xml::escape(date),
            }
        })
        .collect();
    let body = SitemapUrlsetTemplate { urls }.render().unwrap_or_default();
    crate::web::respond_xml(body, XML_CACHE)
}

pub async fn robots(State(state): State<Arc<AppState>>) -> Response {
    let base = state.config.site_url();
    let body = format!(
        "User-agent: *\n\
         Allow: /\n\
         Disallow: /search\n\
         Disallow: /account\n\
         Disallow: /login\n\
         Disallow: /link\n\
         Disallow: /admin\n\
         Disallow: /api/\n\
         Disallow: /auth/\n\
         \n\
         Sitemap: {base}/sitemap.xml\n"
    );
    (
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        body,
    )
        .into_response()
}
