//! Injection-context assembly — structured data for the SessionStart and
//! PreCompact hooks.
//!
//! These types carry the *data* the hook needs; they contain no markdown. The
//! daemon (`kindling-server`) owns the byte-for-byte markdown formatting (so the
//! `toLocaleString`-equivalent date logic lives in exactly one place). This
//! split keeps the service deterministic and trivially testable, and keeps the
//! formatting parity surface contained to one server module.
//!
//! Mirrors the queries performed inline by the Node hooks in
//! `plugins/kindling-claude-code/hooks/{session-start,pre-compact}.js`.

use kindling_types::{Observation, Summary};

/// A pin resolved to its target's content, for injection previews.
///
/// `note` is the pin's reason (Node: `pin.note`); `content` is the *target*
/// observation/summary content the pin points at (Node: `pin.content`, which
/// the TS `listActivePins` join hydrates). Either may be absent: a pin can have
/// no reason, and its target may have been redacted or deleted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPin {
    /// Pin reason / note (`pin.note` in the Node hook). `None` renders as the
    /// literal `Pin` label.
    pub note: Option<String>,
    /// Target content (`pin.content` in the Node hook). `None` renders as
    /// `(no content)`.
    pub content: Option<String>,
}

/// Structured data backing the SessionStart injection.
///
/// Built by [`KindlingService::session_start_context`](crate::KindlingService::session_start_context).
/// The server formats `pins` under a `## Pinned Items` heading and `recent`
/// under `## Recent Activity`, then prepends the Prior-Context header.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionStartContext {
    /// Active pins for the scope, newest first, each resolved to target content.
    pub pins: Vec<ResolvedPin>,
    /// Recent non-redacted observations, newest first, capped at `max_results`.
    pub recent: Vec<Observation>,
}

impl SessionStartContext {
    /// Whether there is anything to inject. The server emits `null`
    /// `additionalContext` when this is empty.
    pub fn is_empty(&self) -> bool {
        self.pins.is_empty() && self.recent.is_empty()
    }
}

/// Structured data backing the PreCompact injection.
///
/// Built by [`KindlingService::pre_compact_context`](crate::KindlingService::pre_compact_context).
/// The server formats `pins` under `## Pinned Items (preserve across
/// compaction)` and the summary under `## Session Summary` (no top-level
/// header).
#[derive(Debug, Clone, PartialEq)]
pub struct PreCompactContext {
    /// Active pins for the scope, newest first, each resolved to target content.
    pub pins: Vec<ResolvedPin>,
    /// The single latest summary across the scope's capsules, if any.
    pub latest_summary: Option<Summary>,
}

impl PreCompactContext {
    /// Whether there is anything to inject. An empty-string summary still counts
    /// as absent, matching the Node hook's `latestSummary.content` truthiness
    /// check — the service guarantees `latest_summary` is `None` in that case.
    pub fn is_empty(&self) -> bool {
        self.pins.is_empty() && self.latest_summary.is_none()
    }
}
