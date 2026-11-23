//! Time `KindlingService::retrieve_at` against an existing database.
//!
//! ```text
//! retrieve_corpus <db> <now_ms> <scope|-> <max_candidates> <reps> <query>...
//! ```
//!
//! `scope` is `session=<id>`, `repo=<id>`, a bare repo id, or `-` for none.
//!
//! Prints one JSON line per query with p50, min and max in milliseconds, the
//! candidate count and the ordered candidate ids, so two builds can be
//! compared for both speed and identical results. The first rep of each query
//! is a warm-up and is not timed. The open is timed separately on stderr,
//! because a writable open runs any pending schema migration.
//!
//! The database is opened writable: point it at a copy you are happy to
//! migrate.
use std::error::Error;
use std::path::Path;
use std::time::Instant;

use kindling_service::KindlingService;
use kindling_types::{RetrieveOptions, RetrievedEntity, ScopeIds};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 7 {
        return Err(
            "usage: retrieve_corpus <db> <now_ms> <session=ID|repo=ID|-> <max_candidates> <reps> <query>..."
                .into(),
        );
    }
    let db = Path::new(&args[1]);
    let now: i64 = args[2].parse()?;
    let scope = match args[3].as_str() {
        "-" => ScopeIds::default(),
        s => match s.split_once('=') {
            Some(("session", id)) => ScopeIds {
                session_id: Some(id.to_string()),
                ..ScopeIds::default()
            },
            Some(("repo", id)) => ScopeIds {
                repo_id: Some(id.to_string()),
                ..ScopeIds::default()
            },
            _ => ScopeIds {
                repo_id: Some(s.to_string()),
                ..ScopeIds::default()
            },
        },
    };
    let max: u32 = args[4].parse()?;
    let reps: usize = args[5].parse()?;
    if reps == 0 {
        return Err("reps must be at least 1".into());
    }

    let started = Instant::now();
    let service = KindlingService::open(db)?;
    eprintln!("open_ms={:.1}", started.elapsed().as_secs_f64() * 1e3);

    for query in &args[6..] {
        let options = RetrieveOptions {
            query: query.clone(),
            scope_ids: scope.clone(),
            token_budget: None,
            max_candidates: Some(max),
            include_redacted: Some(false),
        };
        let mut ids = Vec::new();
        let mut times = Vec::with_capacity(reps);
        for rep in 0..=reps {
            let started = Instant::now();
            let result = service.retrieve_at(options.clone(), now)?;
            let ms = started.elapsed().as_secs_f64() * 1e3;
            if rep == 0 {
                ids = result
                    .candidates
                    .iter()
                    .map(|c| match &c.entity {
                        RetrievedEntity::Observation(o) => o.id.clone(),
                        RetrievedEntity::Summary(s) => s.id.clone(),
                    })
                    .collect();
            } else {
                times.push(ms);
            }
        }
        times.sort_by(f64::total_cmp);
        println!(
            "{}",
            serde_json::json!({
                "query": query,
                "p50_ms": times[times.len() / 2],
                "min_ms": times[0],
                "max_ms": times[times.len() - 1],
                "candidates": ids.len(),
                "ids": ids,
            })
        );
    }
    Ok(())
}
