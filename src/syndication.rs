//! Shared, dependency-free helpers for the XML syndication surface (sitemaps +
//! RSS/Atom feeds).
//!
//! The registry renders these documents from user-supplied package names and
//! descriptions, so two concerns are handled here once, centrally:
//!
//! 1. **`xml::escape`** — escapes the five XML metacharacters *and drops
//!    characters that are illegal in XML 1.0* (most C0 controls). A single
//!    un-stripped control byte from a package description would make the whole
//!    document non-well-formed for every consumer, so this runs on every dynamic
//!    field before it reaches a template (which are declared `escape = "none"`).
//! 2. **`dates`** — converts the schema's canonical `YYYY-MM-DD HH:MM:SS` (UTC)
//!    strings (see [`crate::dal::time`]) into RFC-2822 (RSS `pubDate`) and
//!    RFC-3339 (Atom `updated`). The `time` crate is compiled with only the
//!    `serde` feature here, so the formatting is done by hand.

/// XML text/attribute escaping with XML-1.0 sanitisation.
pub mod xml {
    /// Escape `& < > " '` and drop characters not permitted in an XML 1.0
    /// document (`Char ::= #x9 | #xA | #xD | [#x20-#xD7FF] | [#xE000-#xFFFD] |
    /// [#x10000-#x10FFFF]`). Rust `char`s can never be lone surrogates, so the
    /// surrogate range is excluded for free.
    #[must_use]
    pub fn escape(input: &str) -> String {
        let mut out = String::with_capacity(input.len() + 16);
        for c in input.chars() {
            let legal = matches!(c, '\u{9}' | '\u{A}' | '\u{D}')
                || ('\u{20}'..='\u{D7FF}').contains(&c)
                || ('\u{E000}'..='\u{FFFD}').contains(&c)
                || ('\u{1_0000}'..='\u{10_FFFF}').contains(&c);
            if !legal {
                continue;
            }
            match c {
                '&' => out.push_str("&amp;"),
                '<' => out.push_str("&lt;"),
                '>' => out.push_str("&gt;"),
                '"' => out.push_str("&quot;"),
                '\'' => out.push_str("&apos;"),
                _ => out.push(c),
            }
        }
        out
    }
}

/// Canonical-string → RFC-2822 / RFC-3339 date conversion.
pub mod dates {
    /// Parse a canonical `YYYY-MM-DD HH:MM:SS` string into its six integer
    /// fields, validating the separators. `None` on any deviation.
    fn parse(ts: &str) -> Option<(i32, u8, u8, u8, u8, u8)> {
        let b = ts.as_bytes();
        if ts.len() != 19
            || b[4] != b'-'
            || b[7] != b'-'
            || b[10] != b' '
            || b[13] != b':'
            || b[16] != b':'
        {
            return None;
        }
        Some((
            ts.get(0..4)?.parse().ok()?,
            ts.get(5..7)?.parse().ok()?,
            ts.get(8..10)?.parse().ok()?,
            ts.get(11..13)?.parse().ok()?,
            ts.get(14..16)?.parse().ok()?,
            ts.get(17..19)?.parse().ok()?,
        ))
    }

    /// Convert to an RFC-3339 instant (`2026-07-08T14:30:00Z`) for Atom. Pure
    /// string splice — the `b[10] == ' '` check guarantees byte 10 is a char
    /// boundary, so the slices never split a code point. Falls back to the epoch
    /// (keeping the feed valid) on a malformed input.
    #[must_use]
    pub fn to_rfc3339(ts: &str) -> String {
        let b = ts.as_bytes();
        if ts.len() == 19 && b[10] == b' ' {
            format!("{}T{}Z", &ts[..10], &ts[11..])
        } else {
            "1970-01-01T00:00:00Z".to_string()
        }
    }

    /// Convert to an RFC-2822 date (`Wed, 08 Jul 2026 14:30:00 +0000`) for RSS.
    /// The weekday comes from [`time::Date`]; falls back to the epoch string on a
    /// malformed input so the feed stays valid.
    #[must_use]
    pub fn to_rfc2822(ts: &str) -> String {
        const FALLBACK: &str = "Thu, 01 Jan 1970 00:00:00 +0000";
        const WD: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
        const MON: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let Some((y, mo, d, h, mi, s)) = parse(ts) else {
            return FALLBACK.to_string();
        };
        let Ok(month) = time::Month::try_from(mo) else {
            return FALLBACK.to_string();
        };
        let Ok(date) = time::Date::from_calendar_date(y, month, d) else {
            return FALLBACK.to_string();
        };
        let wd = WD[date.weekday().number_days_from_monday() as usize];
        let mon = MON[(mo - 1) as usize];
        format!("{wd}, {d:02} {mon} {y:04} {h:02}:{mi:02}:{s:02} +0000")
    }
}

/// URL-encoding for `<loc>` / feed link path segments. The canonical site
/// origin comes from [`crate::config::Config::site_url`].
pub mod url {
    /// Percent-encode a string for use in a single URL path segment (or query
    /// value): everything outside the RFC-3986 unreserved set is `%`-encoded.
    /// Defensive — package names are normally already URL-safe, but this ensures
    /// a permissive name (unicode, `&`, space, …) can never corrupt a `<loc>`.
    #[must_use]
    pub fn seg(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        for &byte in s.as_bytes() {
            if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
                out.push(byte as char);
            } else {
                out.push('%');
                out.push(hex(byte >> 4));
                out.push(hex(byte & 0x0f));
            }
        }
        out
    }

    fn hex(nibble: u8) -> char {
        match nibble {
            0..=9 => (b'0' + nibble) as char,
            _ => (b'A' + (nibble - 10)) as char,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_metacharacters() {
        assert_eq!(
            xml::escape(r#"a & b < c > d " e ' f"#),
            "a &amp; b &lt; c &gt; d &quot; e &apos; f"
        );
    }

    #[test]
    fn strips_illegal_control_chars_but_keeps_tab_newline() {
        // \u{0} and \u{7} (bell) are illegal in XML 1.0 → dropped; \t and \n stay.
        assert_eq!(xml::escape("a\u{0}b\u{7}c\td\ne"), "abc\td\ne");
    }

    #[test]
    fn keeps_unicode_and_emoji() {
        assert_eq!(xml::escape("café 🦀"), "café 🦀");
    }

    #[test]
    fn seg_encodes_unsafe_chars() {
        assert_eq!(url::seg("a b&c/d"), "a%20b%26c%2Fd");
        assert_eq!(url::seg("std.json-1_2~x"), "std.json-1_2~x");
        assert_eq!(url::seg("🦀"), "%F0%9F%A6%80");
    }

    #[test]
    fn rfc3339_from_canonical() {
        assert_eq!(
            dates::to_rfc3339("2026-07-08 14:30:00"),
            "2026-07-08T14:30:00Z"
        );
        assert_eq!(dates::to_rfc3339("garbage"), "1970-01-01T00:00:00Z");
        assert_eq!(dates::to_rfc3339(""), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn rfc2822_from_canonical() {
        // 2026-07-08 is a Wednesday.
        assert_eq!(
            dates::to_rfc2822("2026-07-08 14:30:00"),
            "Wed, 08 Jul 2026 14:30:00 +0000"
        );
        // 2000-01-01 is a Saturday.
        assert_eq!(
            dates::to_rfc2822("2000-01-01 00:00:00"),
            "Sat, 01 Jan 2000 00:00:00 +0000"
        );
        assert_eq!(
            dates::to_rfc2822("not-a-date-string!!"),
            "Thu, 01 Jan 1970 00:00:00 +0000"
        );
        // Structurally valid but out-of-range month → fallback, no panic.
        assert_eq!(
            dates::to_rfc2822("2026-13-08 14:30:00"),
            "Thu, 01 Jan 1970 00:00:00 +0000"
        );
    }
}
