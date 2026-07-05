//! Canonical timestamp/date strings, generated in Rust so they are identical
//! on every database engine.
//!
//! The whole schema stores time as TEXT in `YYYY-MM-DD HH:MM:SS` (UTC) and
//! dates as `YYYY-MM-DD`; comparisons rely on that fixed-width, zero-padded
//! form being lexicographically ordered the same as chronologically.

use time::OffsetDateTime;

/// Current UTC timestamp as `YYYY-MM-DD HH:MM:SS`.
pub fn now() -> String {
    fmt_timestamp(OffsetDateTime::now_utc())
}

/// Current UTC date as `YYYY-MM-DD`.
pub fn today() -> String {
    fmt_date(OffsetDateTime::now_utc())
}

/// The UTC date `days` before today, as `YYYY-MM-DD` — used for
/// download-window filters (`download_date >= cutoff`).
pub fn date_days_ago(days: i64) -> String {
    let t = OffsetDateTime::now_utc() - time::Duration::days(days);
    fmt_date(t)
}

fn fmt_timestamp(t: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.year(),
        t.month() as u8,
        t.day(),
        t.hour(),
        t.minute(),
        t.second()
    )
}

fn fmt_date(t: OffsetDateTime) -> String {
    format!("{:04}-{:02}-{:02}", t.year(), t.month() as u8, t.day())
}
