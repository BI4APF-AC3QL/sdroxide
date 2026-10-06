//! Small shared helpers: UTC formatting and text elision.
//!
//! Nothing here knows about [`SdroxideApp`]; these are the bits of formatting
//! that several panels and windows would otherwise each grow their own copy of.

/// Parse `"YYYY-MM-DD"` + `"HH:MM"` (UTC) to a Unix timestamp, falling back to
/// `fallback` if the fields don't fully parse.
pub(in crate::app) fn parse_utc(date: &str, time: &str, fallback: i64) -> i64 {
    let d: Vec<&str> = date.trim().split('-').collect();
    let t: Vec<&str> = time.trim().split(':').collect();
    if d.len() == 3 && t.len() >= 2 {
        if let (Ok(y), Ok(mo), Ok(day), Ok(h), Ok(mi)) =
            (d[0].parse(), d[1].parse(), d[2].parse(), t[0].parse(), t[1].parse())
        {
            return sdroxide_types::ymd_hms_to_unix(y, mo, day, h, mi, 0);
        }
    }
    fallback
}

/// `"YYYY-MM-DD"` for a Unix timestamp (UTC).
pub(in crate::app) fn date_str(unix: i64) -> String {
    let (y, mo, d, ..) = sdroxide_types::utc_ymd_hms(unix);
    format!("{y:04}-{mo:02}-{d:02}")
}

/// `"HH:MM"` for a Unix timestamp (UTC).
pub(in crate::app) fn time_str(unix: i64) -> String {
    let (_, _, _, h, mi, _) = sdroxide_types::utc_ymd_hms(unix);
    format!("{h:02}:{mi:02}")
}

/// Compact age of a spot: `"12s"`, `"3m"`, `"1h"`.
///
/// `pub(crate)` rather than `pub(in crate::app)`: the APRS map is a sibling of
/// the panels and shows exactly the same thing — how long ago something was
/// heard — so it uses the same words for it.
pub(crate) fn fmt_age(secs: i64) -> String {
    let s = secs.max(0);
    if s < 60 {
        crate::language_plugin::format("common.age.text_41_07f67a", "{s}s", &[format!("{s}")])
    } else if s < 3600 {
        { let __lp_arg_0 = &(s / 60); crate::language_plugin::format("common.age.text_43_d8ed65", "{}m", &[format!("{}", __lp_arg_0)]) }
    } else {
        { let __lp_arg_0 = &(s / 3600); crate::language_plugin::format("common.age.text_45_bb1507", "{}h", &[format!("{}", __lp_arg_0)]) }
    }
}

/// A small horizontal signal-activity meter (level ~0..1), so the operator can
/// confirm receive audio is reaching the SSTV decoder.
/// First `n` characters of `text` — a short label that cannot panic on a
/// string shorter than it expects.
pub(in crate::app) fn shorten(text: &str, n: usize) -> &str {
    match text.char_indices().nth(n) {
        Some((end, _)) => &text[..end],
        None => text,
    }
}
