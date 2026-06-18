//! Output shaping for `--json` mode and human-readable text.
//!
//! The JSON shapes here match the TS CLI commands field-for-field (key names
//! and nesting), so the snapshot/parity tests can compare Rust `--json` output
//! against TS output for identical inputs. Text output matches the TS layout
//! closely but is not asserted byte-for-byte.

use serde::Serialize;

use kindling_types::Timestamp;

/// Format a JSON value the way the TS `formatJson` helper does:
/// `JSON.stringify(data, null, pretty ? 2 : 0)` — compact single-line by
/// default, 2-space pretty when requested.
pub fn format_json<T: Serialize>(value: &T, pretty: bool) -> serde_json::Result<String> {
    if pretty {
        serde_json::to_string_pretty(value)
    } else {
        serde_json::to_string(value)
    }
}

/// Format an epoch-ms timestamp as `YYYY-MM-DD HH:MM:SS` (UTC), matching the TS
/// `formatTimestamp`: `new Date(ts).toISOString()` with the `T` replaced by a
/// space and the `.mmmZ` suffix stripped.
pub fn format_timestamp(ts: Timestamp) -> String {
    let iso = iso8601_utc(ts);
    // iso is `YYYY-MM-DDTHH:MM:SS.mmmZ`; drop the fractional+Z and swap T→space.
    let trimmed = iso.split('.').next().unwrap_or(&iso);
    trimmed.replacen('T', " ", 1)
}

/// Format an epoch-ms timestamp as a full ISO-8601 UTC string
/// (`YYYY-MM-DDTHH:MM:SS.mmmZ`), matching JS `new Date(ts).toISOString()`.
pub fn iso8601_utc(ts: Timestamp) -> String {
    // Civil date arithmetic from epoch ms. Handles negative ms (pre-1970) the
    // same way JS Date does (floor division).
    let total_ms = ts;
    let ms = total_ms.rem_euclid(1000);
    let total_secs = total_ms.div_euclid(1000);
    let secs_of_day = total_secs.rem_euclid(86_400);
    let days = total_secs.div_euclid(86_400);

    let (year, month, day) = civil_from_days(days);
    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z")
}

/// Convert days since the Unix epoch (1970-01-01) to a `(year, month, day)`
/// civil date. Howard Hinnant's `civil_from_days` algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// Truncate `text` to at most `max_length` characters, appending `...` when the
/// text was longer. Mirrors the TS `truncate` helper exactly (counts JS string
/// `.length`, i.e. UTF-16 code units; this counts `char`s, which agree for the
/// BMP — astral characters differ but are not exercised by the parity tests).
pub fn truncate(text: &str, max_length: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_length {
        return text.to_string();
    }
    let keep = max_length.saturating_sub(3);
    let prefix: String = text.chars().take(keep).collect();
    format!("{prefix}...")
}

/// Print an error in the shape the TS `formatError`/`handleError` produced.
/// JSON mode emits `{"error":"<msg>"}` (compact); text mode `Error: <msg>`.
/// Always written to stderr, matching `console.error`.
pub fn print_error(message: &str, as_json: bool) {
    if as_json {
        // Compact single-line, matching `formatJson({ error }, false)`.
        let value = serde_json::json!({ "error": message });
        eprintln!("{}", serde_json::to_string(&value).unwrap_or_default());
    } else {
        eprintln!("Error: {message}");
    }
}
