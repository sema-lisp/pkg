//! RSS 2.0 + Atom 1.0 feeds for recently-updated packages and search results.
//!
//! Both feeds map onto an existing bounded, index-backed DAL query so they stay
//! fast at 10M packages: `recent` walks `idx_versions_published`; `search_page`
//! ranks a capped candidate window. Every dynamic value is escaped/date-formatted
//! in Rust (see [`crate::syndication`]) before it reaches the `escape = "none"`
//! templates. The recent feed is served from a single-flight TTL cache (both
//! rendered strings), mirroring [`crate::dal::admin::StatsCache`]; the search
//! feed is per-query so it is uncached and marked `noindex`.

use std::sync::Arc;
use std::time::{Duration, Instant};

use askama::Template;
use axum::{
    extract::{Query, State},
    http::{header, HeaderName},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::dal::packages::ListingRow;
use crate::syndication::{dates, url, xml};
use crate::{dal, AppState};

/// Items in the recently-updated feed.
const RECENT_LIMIT: i64 = 50;
/// Items in a search feed (offset 0; deeper pages are out of scope for a feed).
const SEARCH_LIMIT: i64 = 30;
/// How long a rendered recent feed is reused before recompute.
const RECENT_TTL: Duration = Duration::from_secs(60);

/// One rendered feed entry — every field is already XML-escaped and, for dates,
/// pre-formatted for each syndication format.
pub struct FeedItem {
    pub title: String,
    pub link: String,
    pub id: String,
    pub summary: String,
    pub date_rss: String,
    pub date_atom: String,
}

/// A rendered feed (channel-level metadata + entries). All string fields are
/// render-ready (escaped).
pub struct Feed {
    pub title: String,
    pub site_link: String,
    pub self_link: String,
    pub description: String,
    pub updated_atom: String,
    pub items: Vec<FeedItem>,
}

#[derive(Template)]
#[template(path = "feed_rss.xml", escape = "none")]
struct RssFeedTemplate {
    feed: Feed,
}

#[derive(Template)]
#[template(path = "feed_atom.xml", escape = "none")]
struct AtomFeedTemplate {
    feed: Feed,
}

/// Build a render-ready [`FeedItem`] from a DAL listing row. The GUID/id is
/// version-scoped (`urn:sema-pkg:{name}:{version}`) so a new release of an
/// existing package surfaces as a fresh entry in the recent feed.
fn item_from_row(base: &str, row: ListingRow) -> FeedItem {
    let (name, description, version, published_at) = row;
    let title = if version.is_empty() {
        name.clone()
    } else {
        format!("{name} {version}")
    };
    FeedItem {
        title: xml::escape(&title),
        link: xml::escape(&format!("{base}/packages/{}", url::seg(&name))),
        id: xml::escape(&format!("urn:sema-pkg:{name}:{version}")),
        summary: xml::escape(&description),
        date_rss: dates::to_rfc2822(&published_at),
        date_atom: dates::to_rfc3339(&published_at),
    }
}

/// Assemble a [`Feed`] from listing rows. `<updated>` is the newest item's time
/// (rows arrive newest-first), or "now" for an empty feed.
fn make_feed(
    base: &str,
    self_path: &str,
    title: &str,
    description: &str,
    rows: Vec<ListingRow>,
) -> Feed {
    let items: Vec<FeedItem> = rows.into_iter().map(|r| item_from_row(base, r)).collect();
    let updated_atom = items
        .first()
        .map(|i| i.date_atom.clone())
        .unwrap_or_else(|| dates::to_rfc3339(&crate::dal::time::now()));
    Feed {
        title: xml::escape(title),
        site_link: xml::escape(&format!("{base}/")),
        self_link: xml::escape(&format!("{base}{self_path}")),
        description: xml::escape(description),
        updated_atom,
        items,
    }
}

fn render_rss(feed: Feed) -> String {
    RssFeedTemplate { feed }.render().unwrap_or_default()
}

fn render_atom(feed: Feed) -> String {
    AtomFeedTemplate { feed }.render().unwrap_or_default()
}

// ── Recent feed (cached) ──

const RECENT_TITLE: &str = "Sema Pkg — recently updated";
const RECENT_DESCRIPTION: &str = "Recently published Sema package releases";

/// The rendered recent feeds: `(rss, atom)`.
type RenderedPair = (String, String);
/// Cached feed snapshot: `(computed_at, rendered pair)`.
type FeedSnapshot = (Instant, Arc<RenderedPair>);

/// Single-flight TTL cache holding the rendered `(rss, atom)` recent feeds.
#[derive(Clone, Default)]
pub struct RecentFeedCache {
    inner: Arc<Mutex<Option<FeedSnapshot>>>,
}

impl RecentFeedCache {
    async fn get(&self, state: &AppState) -> Arc<RenderedPair> {
        let mut guard = self.inner.lock().await;
        if let Some((at, cached)) = guard.as_ref() {
            if at.elapsed() < RECENT_TTL {
                return cached.clone();
            }
        }
        let base = state.config.site_url();
        let rows = dal::packages::recent(&state.db, RECENT_LIMIT).await;
        let rss = render_rss(make_feed(
            base,
            "/feed/recent.xml",
            RECENT_TITLE,
            RECENT_DESCRIPTION,
            rows.clone(),
        ));
        let atom = render_atom(make_feed(
            base,
            "/feed/recent.atom",
            RECENT_TITLE,
            RECENT_DESCRIPTION,
            rows,
        ));
        let pair = Arc::new((rss, atom));
        *guard = Some((Instant::now(), pair.clone()));
        pair
    }
}

pub async fn recent_rss(State(state): State<Arc<AppState>>) -> Response {
    let pair = state.recent_feed_cache.get(&state).await;
    crate::web::respond_xml(pair.0.clone(), "public, max-age=300")
}

pub async fn recent_atom(State(state): State<Arc<AppState>>) -> Response {
    let pair = state.recent_feed_cache.get(&state).await;
    crate::web::respond_xml(pair.1.clone(), "public, max-age=300")
}

// ── Search feed (per-query, uncached) ──

#[derive(Deserialize)]
pub struct FeedSearchParams {
    q: Option<String>,
}

/// The rows for a search feed. An empty/whitespace query yields no rows (a valid
/// empty feed) rather than a full scan.
async fn search_rows(state: &AppState, q: &str) -> Vec<ListingRow> {
    if q.trim().is_empty() {
        return Vec::new();
    }
    dal::packages::search_page(&state.db, q, SEARCH_LIMIT, 0).await
}

fn search_feed(base: &str, ext: &str, q: &str, rows: Vec<ListingRow>) -> Feed {
    let title = if q.trim().is_empty() {
        "Sema Pkg — search".to_string()
    } else {
        format!("Sema Pkg — search: {q}")
    };
    let self_path = format!("/feed/search.{ext}?q={}", url::seg(q));
    make_feed(base, &self_path, &title, &title, rows)
}

/// Per-query feed response: short cache and `noindex` (the query space is
/// unbounded, so it must not be crawled/indexed or server-cached).
fn search_response(body: String) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/xml; charset=utf-8"),
            (header::CACHE_CONTROL, "public, max-age=120"),
            (HeaderName::from_static("x-robots-tag"), "noindex"),
        ],
        body,
    )
        .into_response()
}

pub async fn search_rss(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FeedSearchParams>,
) -> Response {
    let q = params.q.unwrap_or_default();
    let base = state.config.site_url();
    let rows = search_rows(&state, &q).await;
    search_response(render_rss(search_feed(base, "xml", &q, rows)))
}

pub async fn search_atom(
    State(state): State<Arc<AppState>>,
    Query(params): Query<FeedSearchParams>,
) -> Response {
    let q = params.q.unwrap_or_default();
    let base = state.config.site_url();
    let rows = search_rows(&state, &q).await;
    search_response(render_atom(search_feed(base, "atom", &q, rows)))
}
