//! `browse` — open a local HTML viewer for kindling memory.

use std::path::{Path, PathBuf};
use std::process::Command;

use kindling_service::ExportBundleOptions;

use crate::cli::BrowseArgs;
use crate::{open_service, CliError, CliResult};

pub fn run(args: BrowseArgs) -> CliResult {
    let (service, db_path) = open_service(args.common.db.as_deref())?;

    let bundle = service.export(ExportBundleOptions {
        scope: None,
        include_redacted: false,
        limit: None,
        metadata: None,
        exported_at: 0,
    })?;

    let data_json = bundle.to_json(false)?;
    let html = render_html(&data_json);

    let output_path = if let Some(path) = &args.output {
        PathBuf::from(path)
    } else {
        std::env::temp_dir().join(format!("kindling-browse-{}.html", std::process::id()))
    };

    if let Some(parent) = output_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(&output_path, html)?;

    if args.no_open {
        println!("{}", output_path.display());
    } else {
        open_in_browser(&output_path)?;
        if !args.common.json {
            println!("Opened {}", output_path.display());
            println!("Database: {}", db_path.display());
        }
    }

    Ok(())
}

fn open_in_browser(path: &Path) -> Result<(), CliError> {
    let path_str = path.to_string_lossy();
    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path_str.as_ref()).status()
    } else if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", path_str.as_ref()])
            .status()
    } else {
        Command::new("xdg-open").arg(path_str.as_ref()).status()
    };

    match status {
        Ok(s) if s.success() => Ok(()),
        Ok(_) => Err(CliError::Invalid(
            "could not open browser (try --no-open and open the file manually)".to_string(),
        )),
        Err(err) => Err(CliError::Invalid(format!(
            "could not open browser: {err} (try --no-open)"
        ))),
    }
}

fn render_html(data_json: &str) -> String {
    // Embed dataset JSON once; the page is fully offline.
    format!(
        r#"<!DOCTYPE html>
<html lang="en-GB">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1" />
  <title>kindling memory</title>
  <style>
    :root {{
      --bg: #0f1419;
      --panel: #1a2332;
      --text: #e7ecf3;
      --muted: #8b9cb3;
      --accent: #5b9fd4;
      --pin: #e8b84a;
      --border: #2a3648;
    }}
    * {{ box-sizing: border-box; }}
    body {{
      margin: 0;
      font-family: ui-sans-serif, system-ui, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.5;
    }}
    header {{
      padding: 1.25rem 1.5rem;
      border-bottom: 1px solid var(--border);
      background: var(--panel);
    }}
    h1 {{ margin: 0; font-size: 1.1rem; font-weight: 600; letter-spacing: 0.02em; }}
    .sub {{ color: var(--muted); font-size: 0.85rem; margin-top: 0.25rem; }}
    main {{ display: grid; grid-template-columns: 280px 1fr; min-height: calc(100vh - 72px); }}
    aside {{
      border-right: 1px solid var(--border);
      padding: 1rem;
      overflow: auto;
    }}
    section {{ padding: 1rem 1.25rem; overflow: auto; }}
    input[type="search"] {{
      width: 100%;
      padding: 0.6rem 0.75rem;
      border: 1px solid var(--border);
      border-radius: 6px;
      background: var(--bg);
      color: var(--text);
      margin-bottom: 0.75rem;
    }}
    .stats {{ display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem; margin-bottom: 1rem; }}
    .stat {{
      background: var(--bg);
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 0.5rem 0.6rem;
      font-size: 0.8rem;
    }}
    .stat strong {{ display: block; font-size: 1.1rem; color: var(--accent); }}
    .list button {{
      display: block;
      width: 100%;
      text-align: left;
      border: 1px solid transparent;
      background: transparent;
      color: var(--text);
      padding: 0.55rem 0.6rem;
      border-radius: 6px;
      cursor: pointer;
      font-size: 0.82rem;
    }}
    .list button:hover, .list button.active {{
      background: var(--bg);
      border-color: var(--border);
    }}
    .badge {{
      display: inline-block;
      font-size: 0.7rem;
      padding: 0.1rem 0.35rem;
      border-radius: 4px;
      background: var(--border);
      color: var(--muted);
      margin-right: 0.35rem;
      text-transform: uppercase;
    }}
    .badge.pin {{ background: #3d3420; color: var(--pin); }}
    .detail h2 {{ margin: 0 0 0.5rem; font-size: 1rem; }}
    .meta {{ color: var(--muted); font-size: 0.82rem; margin-bottom: 1rem; }}
    pre {{
      white-space: pre-wrap;
      word-break: break-word;
      background: var(--panel);
      border: 1px solid var(--border);
      border-radius: 8px;
      padding: 1rem;
      font-size: 0.85rem;
      margin: 0;
    }}
    .empty {{ color: var(--muted); font-size: 0.9rem; }}
    @media (max-width: 800px) {{
      main {{ grid-template-columns: 1fr; }}
      aside {{ border-right: none; border-bottom: 1px solid var(--border); }}
    }}
  </style>
</head>
<body>
  <header>
    <h1>kindling memory</h1>
    <div class="sub">Local viewer · search observations, capsules, and pins</div>
  </header>
  <main>
    <aside>
      <input type="search" id="q" placeholder="Search memory…" autofocus />
      <div class="stats" id="stats"></div>
      <div class="list" id="list"></div>
    </aside>
    <section class="detail" id="detail">
      <p class="empty">Select an item to inspect it.</p>
    </section>
  </main>
  <script>
    const bundle = {data_json};
    const dataset = bundle.dataset || bundle;
    const observations = dataset.observations || [];
    const capsules = dataset.capsules || [];
    const summaries = dataset.summaries || [];
    const pins = dataset.pins || [];
    const pinnedIds = new Set(pins.map(p => p.targetId));

    const items = [
      ...observations.map(o => ({{
        id: o.id, kind: 'observation', type: o.kind, title: o.content.slice(0, 80),
        ts: o.ts, raw: o, pinned: pinnedIds.has(o.id)
      }})),
      ...capsules.map(c => ({{
        id: c.id, kind: 'capsule', type: c.type, title: c.intent,
        ts: c.openedAt, raw: c, pinned: false
      }})),
      ...summaries.map(s => ({{
        id: s.id, kind: 'summary', type: 'summary', title: s.content.slice(0, 80),
        ts: s.createdAt, raw: s, pinned: false
      }})),
    ].sort((a, b) => (b.ts || 0) - (a.ts || 0));

    const statsEl = document.getElementById('stats');
    statsEl.innerHTML = [
      ['Observations', observations.length],
      ['Capsules', capsules.length],
      ['Summaries', summaries.length],
      ['Pins', pins.length],
    ].map(([label, n]) => `<div class="stat"><strong>${{n}}</strong>${{label}}</div>`).join('');

    const listEl = document.getElementById('list');
    const detailEl = document.getElementById('detail');
    const qEl = document.getElementById('q');
    let activeId = null;

    function renderList(filter = '') {{
      const f = filter.toLowerCase();
      const filtered = items.filter(it =>
        !f || it.title.toLowerCase().includes(f) || it.type.includes(f) || it.kind.includes(f)
      );
      listEl.innerHTML = filtered.map(it => `
        <button data-id="${{it.id}}" class="${{activeId === it.id ? 'active' : ''}}">
          ${{it.pinned ? '<span class="badge pin">pin</span>' : ''}}
          <span class="badge">${{it.type}}</span>${{escapeHtml(it.title)}}
        </button>
      `).join('') || '<p class="empty">No matches.</p>';
      listEl.querySelectorAll('button[data-id]').forEach(btn => {{
        btn.addEventListener('click', () => show(btn.dataset.id));
      }});
    }}

    function show(id) {{
      activeId = id;
      const it = items.find(x => x.id === id);
      if (!it) return;
      renderList(qEl.value);
      detailEl.innerHTML = `
        <h2>${{escapeHtml(it.title)}}</h2>
        <div class="meta">${{it.kind}} · ${{it.type}} · ${{formatTs(it.ts)}}</div>
        <pre>${{escapeHtml(JSON.stringify(it.raw, null, 2))}}</pre>
      `;
    }}

    function escapeHtml(s) {{
      return String(s).replace(/[&<>"']/g, c => ({{
        '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;'
      }}[c]));
    }}

    function formatTs(ts) {{
      if (!ts) return 'unknown time';
      return new Date(ts).toLocaleString();
    }}

    qEl.addEventListener('input', () => renderList(qEl.value));
    renderList();
    if (items.length) show(items[0].id);
  </script>
</body>
</html>"#,
        data_json = data_json
    )
}
