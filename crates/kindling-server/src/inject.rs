//! Injection-context markdown formatting — the byte-for-byte parity surface.
//!
//! The daemon (not the hook) owns the markdown so the date logic lives in
//! exactly one place. Two formatters reproduce the Node plugin hooks:
//!
//! - [`format_session_start`] ⇄ `plugins/kindling-claude-code/hooks/session-start.js`
//! - [`format_pre_compact`] ⇄ `plugins/kindling-claude-code/hooks/pre-compact.js`
//!
//! # `toLocaleString` parity
//!
//! The Node hook renders observation timestamps with
//! `new Date(ts).toLocaleString()`. Under the `en-US` locale (our parity
//! target) that yields `M/D/YYYY, H:MM:SS AM/PM`:
//!
//! - month / day / hour: **no** leading zero
//! - minute / second: zero-padded to two digits
//! - 12-hour clock; midnight and noon render as `12`
//! - `AM` / `PM` uppercase, a comma + space between date and time
//!
//! Verified against Node:
//!
//! ```text
//! TZ=America/New_York node -e 'console.log(new Date(1700000000000).toLocaleString("en-US"))'
//! // → 11/14/2023, 5:13:20 PM
//! TZ=UTC          … 0           → 1/1/1970, 12:00:00 AM
//! TZ=UTC          … 1700049600000 → 11/15/2023, 12:00:00 PM
//! ```
//!
//! The timezone is the machine-local zone at the formatted instant (resolved
//! via [`local_offset_seconds`], which honours `TZ` on Unix — the same zone the
//! Node hook process used). [`format_local_datetime`] takes the offset
//! explicitly so the civil-date arithmetic is deterministic and unit-tested with
//! fixed offsets, independent of the host.
//!
//! # UTF-16 truncation
//!
//! Node `String.prototype.substring` counts UTF-16 code units. Previews use
//! [`substring_utf16`], which reproduces that (rounding down at a surrogate-pair
//! boundary, the only divergence — a lone surrogate cannot survive JSON anyway).

use chrono::{Local, TimeZone};
use kindling_service::{PreCompactContext, ResolvedPin, SessionStartContext};

/// SessionStart preview length for pins (UTF-16 code units). Node:
/// `pin.content.substring(0, 200)`.
const SESSION_PIN_PREVIEW: usize = 200;
/// SessionStart preview length for observations. Node:
/// `obs.content.substring(0, 300)`.
const SESSION_OBS_PREVIEW: usize = 300;
/// PreCompact preview length for pins. Node: `pin.content.substring(0, 300)`.
const PRECOMPACT_PIN_PREVIEW: usize = 300;
/// PreCompact summary clamp. Node: `latestSummary.content.substring(0, 500)`.
const PRECOMPACT_SUMMARY_PREVIEW: usize = 500;

/// Header prefixed to the SessionStart injection. Mirrors the Node template
/// literal exactly (note the trailing newline).
const SESSION_HEADER: &str =
    "# Prior Context (from Kindling)\n\nThe following is prior session context for this project:\n";

/// Format the SessionStart `additionalContext`, or `None` when there is nothing
/// to inject (matching the Node hook's "only if ≥1 item" gate).
///
/// `offset_seconds` is the UTC offset to render observation timestamps in (use
/// [`local_offset_seconds`] for the live daemon; a fixed value in tests).
pub fn format_session_start(ctx: &SessionStartContext, offset_seconds: i32) -> Option<String> {
    let mut items: Vec<String> = Vec::new();

    if !ctx.pins.is_empty() {
        items.push("## Pinned Items".to_string());
        for pin in &ctx.pins {
            items.push(format_pin_line(pin, SESSION_PIN_PREVIEW));
        }
    }

    if !ctx.recent.is_empty() {
        items.push("## Recent Activity".to_string());
        for obs in &ctx.recent {
            // `new Date(obs.ts).toLocaleString()`; Node guards `obs.ts ? … : ''`.
            let ts = if obs.ts != 0 {
                format_local_datetime(obs.ts, offset_seconds)
            } else {
                String::new()
            };
            // `(obs.content || '').substring(0,300).replace(/\n/g, ' ')` — replace
            // ALL newlines (the JS regex is global) AFTER truncating.
            let preview = substring_utf16(&obs.content, SESSION_OBS_PREVIEW).replace('\n', " ");
            items.push(format!("- [{ts}] {}: {preview}", obs_kind_str(obs.kind)));
        }
    }

    if items.is_empty() {
        return None;
    }
    Some(format!("{SESSION_HEADER}{}", items.join("\n")))
}

/// Format the PreCompact `additionalContext`, or `None` when there is nothing to
/// inject. No top-level header (matches the Node hook).
pub fn format_pre_compact(ctx: &PreCompactContext) -> Option<String> {
    let mut items: Vec<String> = Vec::new();

    if !ctx.pins.is_empty() {
        items.push("## Pinned Items (preserve across compaction)".to_string());
        for pin in &ctx.pins {
            items.push(format_pin_line(pin, PRECOMPACT_PIN_PREVIEW));
        }
    }

    if let Some(summary) = &ctx.latest_summary {
        // The service already dropped empty-content summaries, mirroring the
        // Node `latestSummary.content` truthiness gate.
        items.push("## Session Summary".to_string());
        items.push(substring_utf16(
            &summary.content,
            PRECOMPACT_SUMMARY_PREVIEW,
        ));
    }

    if items.is_empty() {
        return None;
    }
    Some(items.join("\n"))
}

/// `- **${note || 'Pin'}**: ${content ? content.substring(0, n) : '(no content)'}`
fn format_pin_line(pin: &ResolvedPin, preview_units: usize) -> String {
    let label = pin.note.as_deref().unwrap_or("Pin");
    let preview = match &pin.content {
        Some(content) => substring_utf16(content, preview_units),
        None => "(no content)".to_string(),
    };
    format!("- **{label}**: {preview}")
}

/// The wire/string form of an observation kind, identical to the value stored in
/// the `observations.kind` column and emitted by the Node hook's `obs.kind`.
fn obs_kind_str(kind: kindling_types::ObservationKind) -> &'static str {
    use kindling_types::ObservationKind as K;
    match kind {
        K::ToolCall => "tool_call",
        K::Command => "command",
        K::FileDiff => "file_diff",
        K::Error => "error",
        K::Message => "message",
        K::NodeStart => "node_start",
        K::NodeEnd => "node_end",
        K::NodeOutput => "node_output",
        K::NodeError => "node_error",
    }
}

/// Longest prefix of `s` within `max_units` UTF-16 code units, reproducing JS
/// `String.prototype.substring(0, max_units)`. Rounds down at a surrogate-pair
/// boundary (the lone-surrogate case JS could emit cannot survive JSON anyway).
fn substring_utf16(s: &str, max_units: usize) -> String {
    let mut units = 0usize;
    for (byte_idx, ch) in s.char_indices() {
        let ch_units = ch.len_utf16();
        if units + ch_units > max_units {
            return s[..byte_idx].to_string();
        }
        units += ch_units;
    }
    s.to_string()
}

/// The machine-local UTC offset (seconds) at the instant `epoch_ms`. Honours
/// the `TZ` env var on Unix, so it matches the Node hook process's
/// `toLocaleString()` zone. DST-correct because the offset is resolved *at that
/// instant*, not "now".
pub fn local_offset_seconds(epoch_ms: i64) -> i32 {
    use chrono::Offset;
    let secs = epoch_ms.div_euclid(1000);
    let nanos = (epoch_ms.rem_euclid(1000) * 1_000_000) as u32;
    match Local.timestamp_opt(secs, nanos) {
        chrono::LocalResult::Single(dt) => dt.offset().fix().local_minus_utc(),
        // Ambiguous (fall-back) or skipped (spring-forward) wall-clock instants:
        // pick the earliest candidate. `timestamp_opt` keys on the *UTC* instant
        // which is never ambiguous in practice, so this is belt-and-braces.
        chrono::LocalResult::Ambiguous(dt, _) => dt.offset().fix().local_minus_utc(),
        chrono::LocalResult::None => 0,
    }
}

/// Format `epoch_ms` at a fixed UTC `offset_seconds` as Node's `en-US`
/// `toLocaleString()`: `M/D/YYYY, H:MM:SS AM/PM`.
pub fn format_local_datetime(epoch_ms: i64, offset_seconds: i32) -> String {
    // Shift to local wall-clock seconds, then split into civil date + time of
    // day. All arithmetic is integer and floor-based so negative epochs (pre-1970)
    // behave like JS.
    let local_ms = epoch_ms + (offset_seconds as i64) * 1000;
    let total_secs = local_ms.div_euclid(1000);
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);

    let (year, month, day) = civil_from_days(days);

    let hour24 = (secs_of_day / 3600) as u32;
    let minute = ((secs_of_day % 3600) / 60) as u32;
    let second = (secs_of_day % 60) as u32;

    let (hour12, meridiem) = to_12_hour(hour24);

    // Month / day / hour: no leading zero. Minute / second: zero-padded.
    format!("{month}/{day}/{year}, {hour12}:{minute:02}:{second:02} {meridiem}")
}

/// 24-hour → (12-hour, AM/PM). Midnight and noon render as 12.
fn to_12_hour(hour24: u32) -> (u32, &'static str) {
    let meridiem = if hour24 < 12 { "AM" } else { "PM" };
    let hour12 = match hour24 % 12 {
        0 => 12,
        h => h,
    };
    (hour12, meridiem)
}

/// Civil date `(year, month, day)` from a day count relative to 1970-01-01.
/// Howard Hinnant's `civil_from_days` algorithm (proleptic Gregorian, valid for
/// the full Timestamp range). `month`/`day` are 1-based.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kindling_types::{Observation, ObservationKind, ScopeIds, Summary};
    use serde_json::Map;

    // ---- date formatter (the parity anchor) --------------------------------

    const NY_EST: i32 = -5 * 3600; // America/New_York, standard time (Nov).
    const UTC: i32 = 0;
    const IST: i32 = 5 * 3600 + 30 * 60; // Asia/Kolkata, +05:30.

    #[test]
    fn matches_node_en_us_known_instants() {
        // TZ=America/New_York: 11/14/2023, 5:13:20 PM
        assert_eq!(
            format_local_datetime(1_700_000_000_000, NY_EST),
            "11/14/2023, 5:13:20 PM"
        );
        // TZ=UTC, epoch 0: 1/1/1970, 12:00:00 AM  (midnight → 12, no leading zeros)
        assert_eq!(format_local_datetime(0, UTC), "1/1/1970, 12:00:00 AM");
        // TZ=UTC noon: 11/15/2023, 12:00:00 PM
        assert_eq!(
            format_local_datetime(1_700_049_600_000, UTC),
            "11/15/2023, 12:00:00 PM"
        );
        // TZ=UTC 1am: 11/15/2023, 1:00:00 AM
        assert_eq!(
            format_local_datetime(1_700_010_000_000, UTC),
            "11/15/2023, 1:00:00 AM"
        );
        // TZ=Asia/Kolkata (+05:30): 11/15/2023, 3:43:20 AM
        assert_eq!(
            format_local_datetime(1_700_000_000_000, IST),
            "11/15/2023, 3:43:20 AM"
        );
    }

    #[test]
    fn midnight_and_noon_use_twelve() {
        assert_eq!(to_12_hour(0), (12, "AM"));
        assert_eq!(to_12_hour(12), (12, "PM"));
        assert_eq!(to_12_hour(11), (11, "AM"));
        assert_eq!(to_12_hour(13), (1, "PM"));
        assert_eq!(to_12_hour(23), (11, "PM"));
    }

    #[test]
    fn single_and_double_digit_components() {
        // 2023-01-05 09:07:03 UTC → single-digit month/day/hour, padded min/sec.
        // Compute epoch: days from 1970-01-01 to 2023-01-05.
        let ms = epoch_ms_utc(2023, 1, 5, 9, 7, 3);
        assert_eq!(format_local_datetime(ms, UTC), "1/5/2023, 9:07:03 AM");
        // Double-digit everything just before noon.
        let ms = epoch_ms_utc(2023, 12, 25, 11, 59, 59);
        assert_eq!(format_local_datetime(ms, UTC), "12/25/2023, 11:59:59 AM");
    }

    #[test]
    fn pre_epoch_negative_instant() {
        // TZ=America/New_York, epoch 0 → 12/31/1969, 7:00:00 PM
        assert_eq!(format_local_datetime(0, NY_EST), "12/31/1969, 7:00:00 PM");
    }

    #[test]
    fn civil_from_days_roundtrips_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(-1), (1969, 12, 31));
        // 2000-02-29 (leap day) is day 11016.
        assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    }

    /// Build epoch ms for a UTC civil date/time (test helper; not parity code).
    fn epoch_ms_utc(y: i64, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> i64 {
        let days = days_from_civil(y, m, d);
        (days * 86_400 + (hh as i64) * 3600 + (mm as i64) * 60 + ss as i64) * 1000
    }

    fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
        let y = if m <= 2 { y - 1 } else { y };
        let era = if y >= 0 { y } else { y - 399 } / 400;
        let yoe = y - era * 400;
        let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
        let doy = (153 * mp + 2) / 5 + (d as i64) - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146_097 + doe - 719_468
    }

    // ---- UTF-16 substring ---------------------------------------------------

    #[test]
    fn substring_counts_utf16_units() {
        assert_eq!(substring_utf16("hello", 200), "hello");
        assert_eq!(substring_utf16("hello", 3), "hel");
        // '🦀' is two UTF-16 units. Limit 2 keeps one; limit 1 rounds down to "".
        assert_eq!(substring_utf16("🦀🦀", 2), "🦀");
        assert_eq!(substring_utf16("🦀🦀", 3), "🦀");
        assert_eq!(substring_utf16("🦀🦀", 1), "");
        assert_eq!(substring_utf16("🦀🦀", 4), "🦀🦀");
    }

    // ---- end-to-end markdown ------------------------------------------------

    fn obs(kind: ObservationKind, content: &str, ts: i64) -> Observation {
        Observation {
            id: "o".to_string(),
            kind,
            content: content.to_string(),
            provenance: Map::new(),
            ts,
            scope_ids: ScopeIds::default(),
            redacted: false,
        }
    }

    fn pin(note: Option<&str>, content: Option<&str>) -> ResolvedPin {
        ResolvedPin {
            note: note.map(str::to_string),
            content: content.map(str::to_string),
        }
    }

    #[test]
    fn session_start_full_markdown() {
        let ctx = SessionStartContext {
            pins: vec![
                pin(Some("auth design"), Some("use argon2id")),
                pin(None, None),
            ],
            recent: vec![
                obs(ObservationKind::Command, "git status", 1_700_000_000_000),
                obs(
                    ObservationKind::Message,
                    "line one\nline two",
                    1_700_010_000_000,
                ),
            ],
        };
        let out = format_session_start(&ctx, NY_EST).expect("non-empty");
        let expected = "# Prior Context (from Kindling)\n\n\
The following is prior session context for this project:\n\
## Pinned Items\n\
- **auth design**: use argon2id\n\
- **Pin**: (no content)\n\
## Recent Activity\n\
- [11/14/2023, 5:13:20 PM] command: git status\n\
- [11/14/2023, 8:00:00 PM] message: line one line two";
        assert_eq!(out, expected);
    }

    #[test]
    fn session_start_recent_only() {
        let ctx = SessionStartContext {
            pins: vec![],
            recent: vec![obs(ObservationKind::Error, "boom", 1_700_049_600_000)],
        };
        let out = format_session_start(&ctx, UTC).expect("non-empty");
        let expected = "# Prior Context (from Kindling)\n\n\
The following is prior session context for this project:\n\
## Recent Activity\n\
- [11/15/2023, 12:00:00 PM] error: boom";
        assert_eq!(out, expected);
    }

    #[test]
    fn session_start_zero_ts_renders_empty_bracket() {
        let ctx = SessionStartContext {
            pins: vec![],
            recent: vec![obs(ObservationKind::Message, "hi", 0)],
        };
        let out = format_session_start(&ctx, UTC).expect("non-empty");
        assert!(
            out.ends_with("## Recent Activity\n- [] message: hi"),
            "{out}"
        );
    }

    #[test]
    fn session_start_empty_is_none() {
        let ctx = SessionStartContext {
            pins: vec![],
            recent: vec![],
        };
        assert!(format_session_start(&ctx, UTC).is_none());
    }

    #[test]
    fn pre_compact_full_markdown() {
        let ctx = PreCompactContext {
            pins: vec![pin(Some("keep"), Some("important note"))],
            latest_summary: Some(Summary {
                id: "s".to_string(),
                capsule_id: "c".to_string(),
                content: "we fixed the bug".to_string(),
                confidence: 0.9,
                created_at: 1,
                evidence_refs: vec![],
            }),
        };
        let out = format_pre_compact(&ctx).expect("non-empty");
        let expected = "## Pinned Items (preserve across compaction)\n\
- **keep**: important note\n\
## Session Summary\n\
we fixed the bug";
        assert_eq!(out, expected);
    }

    #[test]
    fn pre_compact_summary_only_no_header() {
        let ctx = PreCompactContext {
            pins: vec![],
            latest_summary: Some(Summary {
                id: "s".to_string(),
                capsule_id: "c".to_string(),
                content: "summary text".to_string(),
                confidence: 1.0,
                created_at: 1,
                evidence_refs: vec![],
            }),
        };
        let out = format_pre_compact(&ctx).expect("non-empty");
        // No "# Prior Context" header on PreCompact.
        assert_eq!(out, "## Session Summary\nsummary text");
    }

    #[test]
    fn pre_compact_empty_is_none() {
        let ctx = PreCompactContext {
            pins: vec![],
            latest_summary: None,
        };
        assert!(format_pre_compact(&ctx).is_none());
    }

    #[test]
    fn pin_preview_truncates_to_unit_limit() {
        let long = "x".repeat(250);
        let line = format_pin_line(&pin(Some("n"), Some(&long)), SESSION_PIN_PREVIEW);
        // 200-unit cap on the preview.
        assert_eq!(line, format!("- **n**: {}", "x".repeat(200)));
    }
}
