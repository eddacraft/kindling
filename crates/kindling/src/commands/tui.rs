//! `tui` — interactive memory browser built with eddacraft-tui.
//!
//! The filtering and ordering live in the pure, terminal-free [`MemoryIndex`]
//! struct (unit-tested), so the TUI loop itself is a thin rendering layer that
//! never runs in CI. When stdin/stdout is not an interactive terminal the
//! command exits with a clear error pointing at `kindling browse`.

use std::collections::HashSet;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use eddacraft_tui::prelude::{
    Action, Binding, Container, EddaCraftTheme, HelpBar, KeyHandler, TerminalGuard, TextInput,
    TextInputState, Theme,
};
use kindling_service::KindlingService;
use kindling_types::{ListObservationsRequest, Observation, PinTargetType, ScopeIds};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    widgets::{Block, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};

use crate::cli::TuiArgs;
use crate::output::format_timestamp;
use crate::{open_service, CliError, CliResult};

/// A terminal-free index of observations plus their pinned status, with
/// deterministic filter/sort logic.
pub struct MemoryIndex {
    observations: Vec<Observation>,
    search_content: Vec<String>,
    labels: Vec<ListItem<'static>>,
}

impl MemoryIndex {
    pub fn new(mut observations: Vec<Observation>, pinned: HashSet<String>) -> Self {
        // The snapshot is immutable: sort and format each observation only once.
        observations.sort_by(|a, b| {
            pinned
                .contains(&b.id)
                .cmp(&pinned.contains(&a.id))
                .then_with(|| b.ts.cmp(&a.ts))
                .then_with(|| a.id.cmp(&b.id))
        });
        let theme = EddaCraftTheme;
        let search_content = observations
            .iter()
            .map(|o| o.content.to_lowercase())
            .collect();
        let labels = observations
            .iter()
            .map(|o| {
                let is_pinned = pinned.contains(&o.id);
                let pin = if is_pinned { " [PIN]" } else { "" };
                // One row per observation keeps viewport calculations independent
                // of embedded newlines in tool output.
                let content = crate::output::truncate(&o.content, 60).replace(['\n', '\r'], " ");
                ListItem::new(format!(
                    "{}  {}  {}{}",
                    format_timestamp(o.ts),
                    kind_str(o.kind),
                    content,
                    pin
                ))
                .style(if is_pinned {
                    theme.title()
                } else {
                    theme.base()
                })
            })
            .collect();
        Self {
            observations,
            search_content,
            labels,
        }
    }

    /// Load every observation (paginated) and the set of pinned observation ids.
    pub fn load(service: &KindlingService) -> Result<Self, CliError> {
        let mut observations = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let page = service.list_observations(ListObservationsRequest {
                scope_ids: ScopeIds::default(),
                kinds: vec![],
                since: None,
                until: None,
                limit: Some(1000),
                cursor,
                include_redacted: None,
            })?;
            let has_more = page.next_cursor.is_some();
            observations.extend(page.observations);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => break,
            }
            if !has_more {
                break;
            }
        }

        let pins = service.list_pins(None)?;
        let pinned = pins
            .iter()
            .filter(|p| p.target_type == PinTargetType::Observation)
            .map(|p| p.target_id.clone())
            .collect();

        Ok(Self::new(observations, pinned))
    }

    /// Number of loaded observations.
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }

    fn matching_indices(&self, query: &str) -> Vec<usize> {
        let q = query.trim().to_lowercase();
        self.search_content
            .iter()
            .enumerate()
            .filter_map(|(i, content)| (q.is_empty() || content.contains(&q)).then_some(i))
            .collect()
    }
}

pub fn run(args: TuiArgs) -> CliResult {
    if args.common.json {
        return Err(CliError::Invalid(
            "kindling tui does not support --json; use kindling list observations --json"
                .to_string(),
        ));
    }
    let (service, _db) = open_service(args.common.db.as_deref())?;
    let index = MemoryIndex::load(&service)?;
    if index.is_empty() {
        println!("No memory to browse. Run `kindling demo` or `kindling log` first.");
        return Ok(());
    }
    run_interactive(&index)
}

/// Run the interactive TUI, or error when the terminal cannot support it.
fn run_interactive(index: &MemoryIndex) -> CliResult {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        return Err(CliError::Invalid(
            "kindling tui requires an interactive terminal; use `kindling browse` for a \
             non-interactive HTML viewer"
                .to_string(),
        ));
    }

    let guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(std::io::stdout());
    let mut terminal = Terminal::new(backend)?;
    let result = event_loop(&mut terminal, index);
    let restored = guard.leave();
    result?;
    restored?;
    Ok(())
}

// Keep visible hints next to the actions consumed by BrowserState.
const BINDINGS: &[Binding] = &[
    Binding {
        keys: "Esc/Ctrl-C",
        action: Action::Quit,
        label: "quit",
    },
    Binding {
        keys: "↑/↓",
        action: Action::Down,
        label: "results",
    },
    Binding {
        keys: "←/→",
        action: Action::Right,
        label: "edit",
    },
    Binding {
        keys: "PgUp/PgDn",
        action: Action::PageDown,
        label: "preview",
    },
];

struct BrowserState {
    query: TextInputState,
    results: ListState,
    matches: Vec<usize>,
    viewport_start: usize,
    preview_scroll: u16,
    preview_max_scroll: u16,
    preview_observation: Option<usize>,
    preview_text: String,
    preview_size: (u16, u16),
}

impl BrowserState {
    fn new(index: &MemoryIndex) -> Self {
        Self {
            query: TextInputState::default(),
            results: ListState::default().with_selected(if index.is_empty() {
                None
            } else {
                Some(0)
            }),
            matches: (0..index.observations.len()).collect(),
            viewport_start: 0,
            preview_scroll: 0,
            preview_max_scroll: 0,
            preview_observation: None,
            preview_text: "no selection".to_string(),
            preview_size: (0, 0),
        }
    }

    fn visible_range(&mut self, height: usize) -> std::ops::Range<usize> {
        if height == 0 {
            return 0..0;
        }
        if let Some(selected) = self.results.selected() {
            if selected < self.viewport_start {
                self.viewport_start = selected;
            } else if selected >= self.viewport_start.saturating_add(height) {
                self.viewport_start = selected + 1 - height;
            }
        }
        self.viewport_start = self
            .viewport_start
            .min(self.matches.len().saturating_sub(height));
        self.viewport_start
            ..self
                .viewport_start
                .saturating_add(height)
                .min(self.matches.len())
    }

    fn prepare_preview(&mut self, index: &MemoryIndex, inner: Rect) {
        let selected = self
            .results
            .selected()
            .and_then(|i| self.matches.get(i))
            .copied();
        let changed = selected != self.preview_observation;
        if changed {
            self.preview_text = selected
                .map(|i| {
                    let o = &index.observations[i];
                    format!(
                        "kind: {}\nid:   {}\n\ntime: {}\n\n{}",
                        kind_str(o.kind),
                        o.id,
                        format_timestamp(o.ts),
                        o.content
                    )
                })
                .unwrap_or_else(|| "no selection".to_string());
            self.preview_observation = selected;
            self.preview_scroll = 0;
        }
        let size = (inner.width, inner.height);
        if changed || size != self.preview_size {
            let lines = Paragraph::new(self.preview_text.as_str())
                .wrap(Wrap { trim: false })
                .line_count(inner.width);
            self.preview_max_scroll = if inner.width == 0 || inner.height == 0 {
                0
            } else {
                u16::try_from(lines.saturating_sub(usize::from(inner.height))).unwrap_or(u16::MAX)
            };
            self.preview_size = size;
        }
        self.preview_scroll = self.preview_scroll.min(self.preview_max_scroll);
    }

    /// Returns true when the user requests exit.
    fn handle_key(&mut self, key: KeyEvent, index: &MemoryIndex) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }
        let mut edited = false;
        let previous_len = self.query.value.len();
        let selected = self.results.selected();
        let action = match key.code {
            KeyCode::PageUp | KeyCode::PageDown => KeyHandler::map(key),
            _ => KeyHandler::map_text_entry(key),
        };
        match action {
            Action::Quit | Action::Back => return true,
            Action::Character(c) => {
                self.query.insert(c);
                edited = true;
            }
            Action::Backspace => {
                self.query.backspace();
                edited = true;
            }
            Action::Delete => {
                self.query.delete();
                edited = true;
            }
            Action::Left => self.query.move_left(),
            Action::Right => self.query.move_right(),
            Action::Home => self.query.home(),
            Action::End => self.query.end(),
            Action::Up => self
                .results
                .select(selected_prev(selected, self.matches.len())),
            Action::Down => self
                .results
                .select(selected_next(selected, self.matches.len())),
            Action::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(5),
            Action::PageDown => {
                self.preview_scroll = self
                    .preview_scroll
                    .saturating_add(5)
                    .min(self.preview_max_scroll)
            }
            _ => {}
        }
        edited &= previous_len != self.query.value.len();
        if edited {
            self.matches = index.matching_indices(&self.query.value);
            self.viewport_start = 0;
            self.results.select(if self.matches.is_empty() {
                None
            } else {
                Some(0)
            });
        }
        if edited || selected != self.results.selected() {
            self.preview_scroll = 0;
        }
        false
    }
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    index: &MemoryIndex,
) -> CliResult {
    let mut browser = BrowserState::new(index);
    loop {
        terminal.draw(|frame| draw(frame, index, &mut browser))?;
        if let Event::Key(key) = event::read()? {
            if browser.handle_key(key, index) {
                return Ok(());
            }
        }
    }
}

fn draw(frame: &mut Frame, index: &MemoryIndex, browser: &mut BrowserState) {
    let theme = EddaCraftTheme;
    let area = frame.area();
    frame.render_widget(Block::default().style(theme.base()), area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);
    let search = TextInput::new(&theme)
        .placeholder("Type to filter memory")
        .block(Container::new(&theme).title("kindling · Search").to_block());
    frame.render_stateful_widget(search, chunks[0], &mut browser.query);

    // Give long observations more room on wide terminals; stack on narrow ones.
    let panes = Layout::default()
        .direction(if area.width >= 100 {
            Direction::Horizontal
        } else {
            Direction::Vertical
        })
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);
    let title = format!("Results ({})", browser.matches.len());
    let block = Container::new(&theme).title(&title).to_block();
    let range = browser.visible_range(usize::from(block.inner(panes[0]).height));
    // Clone only visible cached rows, never the full set of matching labels.
    let rows = range
        .clone()
        .map(|i| index.labels[browser.matches[i]].clone());
    let mut visible_state = ListState::default().with_selected(
        browser
            .results
            .selected()
            .filter(|i| range.contains(i))
            .map(|i| i - range.start),
    );
    frame.render_stateful_widget(
        List::new(rows)
            .block(block)
            .highlight_style(theme.highlighted()),
        panes[0],
        &mut visible_state,
    );

    let preview_block = Container::new(&theme).title("Preview").to_block();
    browser.prepare_preview(index, preview_block.inner(panes[1]));
    frame.render_widget(
        Paragraph::new(browser.preview_text.as_str())
            .style(theme.base())
            .block(preview_block)
            .wrap(Wrap { trim: false })
            .scroll((browser.preview_scroll, 0)),
        panes[1],
    );
    frame.render_widget(HelpBar::new(&theme).bindings(BINDINGS), chunks[2]);
}

fn kind_str(kind: kindling_types::ObservationKind) -> &'static str {
    use kindling_types::ObservationKind::*;
    match kind {
        ToolCall => "tool_call",
        Command => "command",
        FileDiff => "file_diff",
        Error => "error",
        Message => "message",
        NodeStart => "node_start",
        NodeEnd => "node_end",
        NodeOutput => "node_output",
        NodeError => "node_error",
    }
}

fn selected_next(selected: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match selected {
        None => Some(0),
        Some(i) if i + 1 < len => Some(i + 1),
        Some(_) => Some(len - 1),
    }
}

fn selected_prev(selected: Option<usize>, len: usize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    match selected {
        None => Some(len - 1),
        Some(0) => Some(0),
        Some(i) => Some(i - 1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kindling_types::ObservationKind;

    fn search<'a>(index: &'a MemoryIndex, query: &str) -> Vec<&'a Observation> {
        index
            .matching_indices(query)
            .into_iter()
            .map(|i| &index.observations[i])
            .collect()
    }

    fn obs(id: &str, ts: i64, content: &str) -> Observation {
        Observation {
            id: id.to_string(),
            kind: ObservationKind::Message,
            content: content.to_string(),
            provenance: serde_json::Map::new(),
            ts,
            scope_ids: ScopeIds::default(),
            redacted: false,
        }
    }

    #[test]
    fn search_filters_case_insensitively() {
        let index = MemoryIndex::new(
            vec![obs("a", 100, "Hello World"), obs("b", 200, "Goodbye")],
            HashSet::new(),
        );
        let r = search(&index, "hello");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "a");
        assert!(search(&index, "zzz").is_empty());
        assert_eq!(search(&index, "").len(), 2);
    }

    #[test]
    fn search_orders_pinned_first_then_newest() {
        let mut pinned = HashSet::new();
        pinned.insert("old".to_string());
        let index = MemoryIndex::new(
            vec![
                obs("old", 100, "x"),
                obs("new", 300, "x"),
                obs("mid", 200, "x"),
            ],
            pinned,
        );
        let ids: Vec<&str> = search(&index, "x").iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["old", "new", "mid"]);
    }

    #[test]
    fn cached_labels_mark_only_pinned_observations() {
        use ratatui::backend::TestBackend;
        let index = MemoryIndex::new(
            vec![obs("p", 1, "pinned"), obs("q", 2, "ordinary")],
            HashSet::from(["p".to_string()]),
        );
        let mut browser = BrowserState::new(&index);
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal
            .draw(|frame| draw(frame, &index, &mut browser))
            .unwrap();
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert_eq!(text.matches("[PIN]").count(), 1);
        assert!(text.contains("ordinary"));
    }

    #[test]
    fn selection_navigation_clamps() {
        assert_eq!(selected_next(None, 0), None);
        assert_eq!(selected_next(Some(0), 3), Some(1));
        assert_eq!(selected_next(Some(2), 3), Some(2));
        assert_eq!(selected_prev(Some(0), 3), Some(0));
        assert_eq!(selected_prev(Some(1), 3), Some(0));
    }

    #[test]
    fn search_accepts_quit_and_navigation_letters() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let index = MemoryIndex::new(vec![obs("a", 1, "qhjkl query")], HashSet::new());
        let mut browser = BrowserState::new(&index);
        for c in "qhjkl query".chars() {
            assert!(
                !browser.handle_key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE), &index)
            );
        }
        assert_eq!(browser.query.value, "qhjkl query");
        assert_eq!(browser.results.selected(), Some(0));
        assert!(browser.handle_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &index));
        assert!(browser.handle_key(
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
            &index
        ));
    }

    #[test]
    fn editing_search_restores_selection_after_no_matches() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let index = MemoryIndex::new(vec![obs("a", 1, "query")], HashSet::new());
        let mut browser = BrowserState::new(&index);
        for code in [KeyCode::Char('z'), KeyCode::Backspace] {
            browser.handle_key(KeyEvent::new(code, KeyModifiers::NONE), &index);
            assert_eq!(
                browser.results.selected(),
                if browser.query.value.is_empty() {
                    Some(0)
                } else {
                    None
                }
            );
        }
        for code in [
            KeyCode::Char('q'),
            KeyCode::Char('é'),
            KeyCode::Left,
            KeyCode::Delete,
        ] {
            browser.handle_key(KeyEvent::new(code, KeyModifiers::NONE), &index);
        }
        assert_eq!(browser.query.value, "q");
        assert_eq!(browser.results.selected(), Some(0));
    }

    #[test]
    fn changing_selection_resets_preview_scroll() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let index = MemoryIndex::new(
            vec![obs("a", 1, "a"), obs("b", 2, &"preview ".repeat(50))],
            HashSet::new(),
        );
        let mut browser = BrowserState::new(&index);
        browser.prepare_preview(&index, Rect::new(0, 0, 20, 3));
        browser.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &index);
        assert!(browser.preview_scroll > 0);
        browser.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &index);
        assert_eq!(browser.preview_scroll, 0);
        assert_eq!(browser.results.selected(), Some(1));
    }

    #[test]
    fn browser_renders_wide_narrow_and_tiny_terminals() {
        use ratatui::backend::TestBackend;
        let index = MemoryIndex::new(vec![obs("a", 1, "query result")], HashSet::new());
        for (width, height) in [(120, 30), (80, 24), (20, 6), (1, 1)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            let mut browser = BrowserState::new(&index);
            terminal
                .draw(|frame| draw(frame, &index, &mut browser))
                .unwrap();
            if width >= 80 {
                let buffer = terminal.backend().buffer();
                let text: String = buffer.content().iter().map(|cell| cell.symbol()).collect();
                assert!(text.contains("Search"));
                assert!(text.contains("Results (1)"));
                assert!(text.contains("query result"));
                assert!(text.contains("quit"));
            }
        }
    }

    #[test]
    fn preview_scroll_is_clamped_after_page_down_and_resize() {
        use crossterm::event::KeyModifiers;
        use ratatui::backend::TestBackend;
        let index = MemoryIndex::new(
            vec![obs("a", 1, &"wrapped words ".repeat(100))],
            HashSet::new(),
        );
        let mut browser = BrowserState::new(&index);
        let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
        terminal
            .draw(|frame| draw(frame, &index, &mut browser))
            .unwrap();
        for _ in 0..100 {
            browser.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE), &index);
            terminal
                .draw(|frame| draw(frame, &index, &mut browser))
                .unwrap();
        }
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            text.contains("wrapped words"),
            "scrolling must leave the final page visible"
        );
        terminal.backend_mut().resize(200, 100);
        terminal
            .resize(ratatui::layout::Rect::new(0, 0, 200, 100))
            .unwrap();
        terminal
            .draw(|frame| draw(frame, &index, &mut browser))
            .unwrap();
        assert_eq!(
            browser.preview_scroll, 0,
            "growing the pane must remove obsolete scroll"
        );
    }

    #[test]
    fn navigation_reuses_matches_and_renders_the_selected_viewport() {
        use crossterm::event::KeyModifiers;
        use ratatui::backend::TestBackend;
        let observations = (0..1000)
            .map(|i| obs(&i.to_string(), i, &format!("row {i}")))
            .collect();
        let index = MemoryIndex::new(observations, HashSet::new());
        let mut browser = BrowserState::new(&index);
        let matches = browser.matches.as_ptr();
        let labels = index.labels.as_ptr();
        let mut terminal = Terminal::new(TestBackend::new(120, 20)).unwrap();
        for _ in 0..999 {
            browser.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &index);
        }
        terminal
            .draw(|frame| draw(frame, &index, &mut browser))
            .unwrap();
        assert_eq!(browser.results.selected(), Some(999));
        assert_eq!(browser.matches.as_ptr(), matches);
        assert_eq!(index.labels.as_ptr(), labels);
        assert!(browser.visible_range(14).len() <= 14);
        let text: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(
            text.contains("row 0"),
            "the last selected observation must be visible"
        );
        browser.handle_key(
            KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE),
            &index,
        );
        terminal
            .draw(|frame| draw(frame, &index, &mut browser))
            .unwrap();
        assert!(browser.matches.is_empty());
        assert!(browser.results.selected().is_none());
    }
}
