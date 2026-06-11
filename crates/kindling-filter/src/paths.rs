//! Excluded-path filtering.
//!
//! Ports `EXCLUDED_PATHS` / `isExcludedPath` from
//! `packages/kindling-adapter-claude-code/src/claude-code/filter.ts` (the
//! plugin hook filter has no path rules; the adapter set is the canonical
//! one and is identical in the OpenCode adapter).

use std::sync::OnceLock;

use regex::Regex;

fn excluded_paths() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            r"node_modules",
            r"\.git/",
            r"\.env$",
            r"\.pem$",
            r"\.key$",
            r"(?i)credentials",
            r"(?i)secrets",
        ]
        .iter()
        .map(|pattern| Regex::new(pattern).expect("excluded-path pattern compiles"))
        .collect()
    })
}

/// True if a file path should be excluded from capture.
pub fn is_excluded_path(path: &str) -> bool {
    excluded_paths()
        .iter()
        .any(|pattern| pattern.is_match(path))
}
