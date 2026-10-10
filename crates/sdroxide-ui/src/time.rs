//! Wall clock, on both targets.
//!
//! `SystemTime::now()` panics on `wasm32-unknown-unknown` — there is no clock
//! behind it — so the browser build asks JavaScript instead. Every part of the
//! UI that needs the date rather than a frame time goes through here: the
//! logbook, the waterfall's time gridlines, and the solar view, whose whole
//! scene is a function of an explicit timestamp.

/// Current Unix time (UTC seconds).
pub fn now_unix() -> i64 {
    now_unix_f64() as i64
}

/// Current Unix time as fractional UTC seconds.
pub fn now_unix_f64() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now() / 1000.0
    }
}

/// The operator's current offset from UTC, in seconds east of Greenwich.
pub fn local_offset_seconds() -> i64 {
    local_offset_at(now_unix())
}

/// The operator's offset from UTC at `unix`, in seconds east of Greenwich.
///
/// Both halves ask the platform rather than deriving anything: the offset is a
/// political fact about that instant — which side of a DST boundary it falls
/// on — and not something a Unix timestamp carries. Asked per instant, so a
/// satellite pass after tonight's clock change reads in tomorrow's zone.
pub fn local_offset_at(unix: i64) -> i64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        // chrono re-reads the zone at most once a second and caches it, so this
        // is cheap enough to call once a frame.
        use chrono::{Offset, TimeZone};
        chrono::Local
            .timestamp_opt(unix, 0)
            .single()
            .map_or(0, |t| i64::from(t.offset().fix().local_minus_utc()))
    }
    #[cfg(target_arch = "wasm32")]
    {
        // getTimezoneOffset counts minutes *behind* local time, so UTC+2 — two
        // hours east — reports -120.
        let at = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(unix as f64 * 1000.0));
        let minutes = at.get_timezone_offset();
        (-minutes * 60.0) as i64
    }
}
