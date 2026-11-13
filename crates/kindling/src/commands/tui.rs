//! `tui` — interactive ratatui memory browser.
//!
//! The filtering and ordering live in the pure, terminal-free [`MemoryIndex`]
//! struct (unit-tested), so the TUI loop itself is a thin rendering layer that
//! never runs in CI. When stdin/stdout is not an interactive terminal the
//! command exits with a clear error pointing at `kindling browse`.

use std::collections::HashSet;

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use kindling_service::KindlingService;
use kindling_types::{ListObservationsRequest, Observation, PinTargetType, ScopeIds};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::Span,
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph},
    Terminal,
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
    if !std::io::stdout().is_terminal() {
        return Err(CliError::Invalid(
            "kindling tui requires an interactive terminal; use `kindling browse` for a \
             non-interactive HTML viewer"
                .to_string(),
        ));
    }

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = event_loop(&mut terminal, index);

    // Always restore the terminal, even if the loop returned an error.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn event_loop(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    index: &MemoryIndex,
) -> CliResult {
    let mut query = String::new();
    let mut state = ListState::default();
    state.select(Some(0));

    loop {
        let items = index.search(&query);
        draw(terminal, index, &query, &items, &mut state)?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Esc => break,
                KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => break,
                KeyCode::Char(c) => {
                    query.push(c);
                    state.select(if items.is_empty() { None } else { Some(0) });
                }
                KeyCode::Backspace => {
                    query.pop();
                    state.select(if items.is_empty() { None } else { Some(0) });
                }
                KeyCode::Up => state.select(selected_prev(state.selected(), items.len())),
                KeyCode::Down => state.select(selected_next(state.selected(), items.len())),
                _ => {}
            }
        }
    }
    Ok(())
}

fn draw(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    index: &MemoryIndex,
    query: &str,
    items: &[&Observation],
    state: &mut ListState,
) -> Result<(), CliError> {
    terminal.draw(|f| {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Length(3),
                Constraint::Min(4),
                Constraint::Length(10),
            ])
            .split(f.area());

        let prompt = Span::styled(
            format!("> {query}"),
            Style::default().add_modifier(Modifier::BOLD),
        );
        let search = Paragraph::new(prompt).block(
            Block::default()
                .borders(Borders::ALL)
                .title("Search (type to filter, q to quit)"),
        );
        f.render_widget(search, chunks[0]);

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
            })
            .collect();
        let list = List::new(list_items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!("Results ({})", items.len())),
            )
            .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
        f.render_stateful_widget(list, chunks[1], state);

        let preview = state
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
        let preview =
            Paragraph::new(preview).block(Block::default().borders(Borders::ALL).title("Preview"));
        f.render_widget(preview, chunks[2]);
    })?;
    Ok(())
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
}
