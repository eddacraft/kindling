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
    layout::{Constraint, Direction, Layout},
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
    pinned: HashSet<String>,
}

impl MemoryIndex {
    pub fn new(observations: Vec<Observation>, pinned: HashSet<String>) -> Self {
        Self {
            observations,
            pinned,
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

    pub fn is_pinned(&self, id: &str) -> bool {
        self.pinned.contains(id)
    }

    /// Filter observations by a case-insensitive substring of their content
    /// and order them pinned-first, then newest-first.
    pub fn search(&self, query: &str) -> Vec<&Observation> {
        let q = query.trim().to_lowercase();
        let mut matches: Vec<&Observation> = self
            .observations
            .iter()
            .filter(|o| q.is_empty() || o.content.to_lowercase().contains(&q))
            .collect();
        matches.sort_by(|a, b| {
            self.pinned
                .contains(&b.id)
                .cmp(&self.pinned.contains(&a.id))
                .then_with(|| b.ts.cmp(&a.ts))
        });
        matches
    }
}

pub fn run(args: TuiArgs) -> CliResult {
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
    preview_scroll: u16,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            query: TextInputState::default(),
            results: ListState::default().with_selected(Some(0)),
            preview_scroll: 0,
        }
    }
}

impl BrowserState {
    /// Returns true when the user requests exit.
    fn handle_key(&mut self, key: KeyEvent, index: &MemoryIndex) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }
        let mut edited = false;
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
            Action::Up => self.results.select(selected_prev(
                selected,
                index.search(&self.query.value).len(),
            )),
            Action::Down => self.results.select(selected_next(
                selected,
                index.search(&self.query.value).len(),
            )),
            Action::PageUp => self.preview_scroll = self.preview_scroll.saturating_sub(5),
            Action::PageDown => self.preview_scroll = self.preview_scroll.saturating_add(5),
            _ => {}
        }
        if edited {
            // Recompute after editing: deleting the final non-matching character
            // must restore the preview and selection immediately.
            self.results
                .select(if index.search(&self.query.value).is_empty() {
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
    let mut browser = BrowserState::default();
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
    let items = index.search(&browser.query.value);
    let list_items: Vec<ListItem> = items
        .iter()
        .map(|o| {
            let pin = if index.is_pinned(&o.id) { " [PIN]" } else { "" };
            ListItem::new(format!(
                "{}  {}  {}{}",
                format_timestamp(o.ts),
                kind_str(o.kind),
                crate::output::truncate(&o.content, 60),
                pin,
            ))
            .style(if index.is_pinned(&o.id) {
                theme.title()
            } else {
                theme.base()
            })
        })
        .collect();
    let title = format!("Results ({})", items.len());
    let list = List::new(list_items)
        .block(Container::new(&theme).title(&title).to_block())
        .highlight_style(theme.highlighted());
    frame.render_stateful_widget(list, panes[0], &mut browser.results);

    let preview = browser
        .results
        .selected()
        .and_then(|i| items.get(i))
        .map(|o| {
            format!(
                "kind: {}\nid:   {}\n\ntime: {}\n\n{}",
                kind_str(o.kind),
                o.id,
                format_timestamp(o.ts),
                o.content,
            )
        })
        .unwrap_or_else(|| "no selection".to_string());
    frame.render_widget(
        Paragraph::new(preview)
            .style(theme.base())
            .block(Container::new(&theme).title("Preview").to_block())
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
        let r = index.search("hello");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].id, "a");
        assert!(index.search("zzz").is_empty());
        assert_eq!(index.search("").len(), 2);
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
        let ids: Vec<&str> = index.search("x").iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids, vec!["old", "new", "mid"]);
    }

    #[test]
    fn is_pinned_reports_membership() {
        let mut pinned = HashSet::new();
        pinned.insert("p".to_string());
        let index = MemoryIndex::new(vec![obs("p", 1, "x")], pinned);
        assert!(index.is_pinned("p"));
        assert!(!index.is_pinned("q"));
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
        let mut browser = BrowserState::default();
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
        let mut browser = BrowserState::default();
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
        let index = MemoryIndex::new(vec![obs("a", 1, "a"), obs("b", 2, "b")], HashSet::new());
        let mut browser = BrowserState::default();
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
            let mut browser = BrowserState::default();
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
}
