//! Length truncation.
//!
//! Ports `truncate` from `plugins/kindling-claude-code/hooks/lib/filter.js`.
//! JavaScript measures string length in UTF-16 code units, so this module
//! counts UTF-16 units (not bytes, not chars) to keep limits and the
//! `[Truncated N chars]` notice byte-for-byte identical for any content that
//! JavaScript can emit as valid JSON. The one divergence: when the limit
//! falls inside a surrogate pair, JS `substring` emits a lone surrogate
//! (which cannot survive a JSON round-trip anyway); Rust rounds down to the
//! previous character boundary.

/// Maximum content length before truncation (UTF-16 code units).
pub const MAX_CONTENT_LENGTH: usize = 10_000;

/// Tighter limit applied to noisy tools' results.
pub const NOISY_TOOL_MAX_LENGTH: usize = 2_000;

/// Truncate content to `max_length` UTF-16 code units, appending the same
/// notice as the Node.js filter when content was cut.
pub fn truncate(content: &str, max_length: usize) -> String {
    let (prefix, total_units) = utf16_prefix(content, max_length);
    if total_units <= max_length {
        return content.to_string();
    }
    format!("{prefix}\n[Truncated {} chars]", total_units - max_length)
}

/// Longest prefix of `content` that fits in `max_units` UTF-16 code units
/// without splitting a surrogate pair, plus the total UTF-16 length.
fn utf16_prefix(content: &str, max_units: usize) -> (&str, usize) {
    let mut units = 0;
    for (byte_idx, ch) in content.char_indices() {
        let ch_units = ch.len_utf16();
        if units + ch_units > max_units {
            let total = units
                + content[byte_idx..]
                    .chars()
                    .map(char::len_utf16)
                    .sum::<usize>();
            return (&content[..byte_idx], total);
        }
        units += ch_units;
    }
    (content, units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_is_unchanged() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("", 10), "");
    }

    #[test]
    fn content_at_exactly_the_limit_is_unchanged() {
        let content = "x".repeat(10);
        assert_eq!(truncate(&content, 10), content);
    }

    #[test]
    fn long_content_gets_the_node_notice() {
        let content = "x".repeat(15);
        assert_eq!(
            truncate(&content, 10),
            format!("{}\n[Truncated 5 chars]", "x".repeat(10))
        );
    }

    #[test]
    fn length_counts_utf16_units_like_javascript() {
        // '🦀' is one char but two UTF-16 units; JS sees "🦀🦀🦀".length === 6.
        let content = "🦀🦀🦀";
        assert_eq!(truncate(content, 6), content);
        // Limit 4 keeps two crabs and reports 2 units cut, like JS would.
        assert_eq!(truncate(content, 4), "🦀🦀\n[Truncated 2 chars]");
        // Limit 5 falls inside the third crab's surrogate pair: the prefix
        // rounds down to a char boundary but the notice still reports the
        // UTF-16 count JS would (6 - 5 = 1).
        assert_eq!(truncate(content, 5), "🦀🦀\n[Truncated 1 chars]");
    }
}
