//! Daemon read/write contention measurement (PERF-002).
//!
//! Starts a real `kindling serve` child process on an isolated kindling home,
//! seeds one project with a large corpus in which a common word appears in
//! most rows, and then measures, over the HTTP-over-UDS client path:
//!
//! - **idle**: hook-style observation appends and a small (rare-term) retrieve
//!   with no other load, and
//! - **contended**: the same appends and small retrieves while `--readers`
//!   tasks run broad retrieves for the common word back to back against the
//!   same project.
//!
//! Prints a JSON report with p50/p95/max per operation and phase. The child
//! daemon is stopped by its own PID; nothing outside the temp dirs is touched.
//!
//! ```text
//! cargo build --release -p eddacraft-kindling --bin kindling
//! cargo run --release -p kindling-bench --bin daemon_contention -- \
//!     --kindling-bin target/release/kindling
//! ```

use std::error::Error;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kindling_bench::statistics::LatencySummary;
use kindling_client::{Client, ClientConfig, Spawner, Transport, EXPECTED_SCHEMA_VERSION};
use kindling_service::KindlingService;
use kindling_types::{ObservationInput, ObservationKind, RetrieveOptions, ScopeIds};
use serde::Serialize;

type BoxError = Box<dyn Error + Send + Sync>;

const PROJECT_ROOT: &str = "contention-project";
const COMMON_WORD: &str = "build";
const RARE_WORD: &str = "zebrafish";

#[derive(Debug, Clone)]
struct Args {
    kindling_bin: PathBuf,
    rows: usize,
    common_percent: u32,
    readers: usize,
    phase_secs: u64,
    interval_ms: u64,
    home_dir: Option<PathBuf>,
}

impl Args {
    fn parse() -> Result<Self, BoxError> {
        let mut args = Args {
            kindling_bin: std::env::var_os("KINDLING_BIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("target/release/kindling")),
            rows: 120_000,
            common_percent: 60,
            readers: 2,
            phase_secs: 10,
            interval_ms: 20,
            home_dir: None,
        };
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            let mut value = || it.next().ok_or_else(|| format!("{flag} needs a value"));
            match flag.as_str() {
                "--kindling-bin" => args.kindling_bin = PathBuf::from(value()?),
                "--rows" => args.rows = value()?.parse()?,
                "--common-percent" => args.common_percent = value()?.parse()?,
                "--readers" => args.readers = value()?.parse()?,
                "--phase-secs" => args.phase_secs = value()?.parse()?,
                "--interval-ms" => args.interval_ms = value()?.parse()?,
                "--home-dir" => args.home_dir = Some(PathBuf::from(value()?)),
                "-h" | "--help" => {
                    println!(
                        "daemon_contention [--kindling-bin PATH] [--rows N] [--common-percent P] \
                         [--readers R] [--phase-secs S] [--interval-ms MS] [--home-dir DIR]"
                    );
                    std::process::exit(0);
                }
                other => return Err(format!("unknown flag {other}").into()),
            }
        }
        Ok(args)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    rows: usize,
    common_percent: u32,
    readers: usize,
    phase_secs: u64,
    interval_ms: u64,
    seed_secs: f64,
    db_bytes: u64,
    phases: Vec<Phase>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Phase {
    name: String,
    append: LatencySummary,
    small_retrieve: LatencySummary,
    broad_retrieve: Option<LatencySummary>,
}

/// Stops the child daemon by its own PID when dropped.
struct DaemonChild(Child);

impl Drop for DaemonChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn main() -> Result<(), BoxError> {
    let args = Args::parse()?;
    if !args.kindling_bin.is_file() {
        return Err(format!(
            "kindling binary not found at {} (build it with `cargo build --release -p \
             eddacraft-kindling --bin kindling` or pass --kindling-bin)",
            args.kindling_bin.display()
        )
        .into());
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let report = runtime.block_on(run(args))?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

async fn run(args: Args) -> Result<Report, BoxError> {
    // The kindling home holds the (large) database; `--home-dir` picks where
    // it lives. The socket goes in its own short temp dir: the UDS path limit
    // is ~108 bytes.
    let home = match &args.home_dir {
        Some(dir) => tempfile::Builder::new().prefix("kc").tempdir_in(dir)?,
        None => tempfile::Builder::new().prefix("kc").tempdir()?,
    };
    let socket_dir = tempfile::Builder::new().prefix("kcs").tempdir()?;
    let socket = socket_dir.path().join("k.sock");

    let child = Command::new(&args.kindling_bin)
        .arg("serve")
        .arg("--kindling-home")
        .arg(home.path())
        .arg("--socket")
        .arg(&socket)
        .arg("--idle-timeout")
        .arg("3600")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()?;
    let _daemon = DaemonChild(child);
    let client = attach_client(&socket);
    wait_ready(&client).await?;

    // Touch the project so the daemon creates and migrates its database, then
    // find the database path from the reported project id.
    client
        .append_observation(observation(0, "warmup"), None, None)
        .await?;
    let health = client.health().await?;
    let project_id = health
        .projects
        .first()
        .ok_or("daemon reported no project after the first write")?
        .clone();
    let db_path = home
        .path()
        .join("projects")
        .join(project_id)
        .join("kindling.db");

    let seed_started = Instant::now();
    let seed_args = args.clone();
    let seed_path = db_path.clone();
    tokio::task::spawn_blocking(move || seed(&seed_path, &seed_args)).await??;
    let seed_secs = seed_started.elapsed().as_secs_f64();
    eprintln!("seeded {} rows in {seed_secs:.1}s", args.rows);

    // Warm the page cache and the daemon's connections.
    for _ in 0..3 {
        client.retrieve(broad_query()).await?;
        client.retrieve(small_query()).await?;
    }

    let idle = phase(&client, &args, "idle", 0).await?;
    let contended = phase(&client, &args, "contended", args.readers).await?;

    Ok(Report {
        rows: args.rows,
        common_percent: args.common_percent,
        readers: args.readers,
        phase_secs: args.phase_secs,
        interval_ms: args.interval_ms,
        seed_secs,
        db_bytes: file_size(&db_path) + file_size(&db_path.with_extension("db-wal")),
        phases: vec![idle, contended],
    })
}

fn attach_client(socket: &Path) -> Client {
    Client::with_config(ClientConfig {
        socket_path: socket.to_path_buf(),
        port_path: socket.with_file_name("kindling.port"),
        project_root: PROJECT_ROOT.into(),
        expected_schema_version: EXPECTED_SCHEMA_VERSION,
        connect_timeout: Duration::from_secs(5),
        poll_interval: Duration::from_millis(10),
        // Attach only: the benchmark owns the daemon process.
        spawn: Spawner::custom(|| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "daemon_contention attaches to its own child daemon only",
            ))
        }),
        transport: Transport::default(),
        spawn_log_path: None,
    })
}

async fn wait_ready(client: &Client) -> Result<(), BoxError> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match client.health().await {
            Ok(_) => return Ok(()),
            Err(err) if Instant::now() > deadline => return Err(err.into()),
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
}

/// Run one measurement phase: a paced appender and a paced small-retrieve task,
/// plus `readers` back-to-back broad retrieves.
async fn phase(
    client: &Client,
    args: &Args,
    name: &str,
    readers: usize,
) -> Result<Phase, BoxError> {
    let stop = Arc::new(AtomicBool::new(false));
    let interval = Duration::from_millis(args.interval_ms);

    let mut broad_tasks = Vec::new();
    for _ in 0..readers {
        let client = client.clone();
        let stop = Arc::clone(&stop);
        broad_tasks.push(tokio::spawn(async move {
            let mut samples = Vec::new();
            while !stop.load(Ordering::Relaxed) {
                let started = Instant::now();
                client.retrieve(broad_query()).await?;
                samples.push(started.elapsed());
            }
            Ok::<_, BoxError>(samples)
        }));
    }
    // Let the broad readers saturate before measuring.
    if readers > 0 {
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    let phase_started = Instant::now();
    let deadline = phase_started + Duration::from_secs(args.phase_secs);

    let append_client = client.clone();
    let appender = tokio::spawn(async move {
        let mut samples = Vec::new();
        let mut index = 10_000_000usize;
        while Instant::now() < deadline {
            index += 1;
            let started = Instant::now();
            append_client
                .append_observation(observation(index, "hook"), None, None)
                .await?;
            samples.push(started.elapsed());
            tokio::time::sleep(interval).await;
        }
        Ok::<_, BoxError>(samples)
    });

    let small_client = client.clone();
    let small = tokio::spawn(async move {
        let mut samples = Vec::new();
        while Instant::now() < deadline {
            let started = Instant::now();
            small_client.retrieve(small_query()).await?;
            samples.push(started.elapsed());
            tokio::time::sleep(interval).await;
        }
        Ok::<_, BoxError>(samples)
    });

    let append_samples = appender.await??;
    let small_samples = small.await??;
    let elapsed = phase_started.elapsed();
    stop.store(true, Ordering::Relaxed);

    let mut broad_samples = Vec::new();
    for task in broad_tasks {
        broad_samples.extend(task.await??);
    }

    Ok(Phase {
        name: name.into(),
        append: LatencySummary::from_samples(&append_samples, elapsed),
        small_retrieve: LatencySummary::from_samples(&small_samples, elapsed),
        broad_retrieve: (readers > 0)
            .then(|| LatencySummary::from_samples(&broad_samples, elapsed)),
    })
}

/// Seed the project directly through the in-process service (fast, no HTTP).
fn seed(db_path: &Path, args: &Args) -> Result<(), BoxError> {
    let service = KindlingService::open(db_path)?;
    for index in 1..=args.rows {
        service.append_observation(seed_observation(index, args), Default::default())?;
    }
    Ok(())
}

fn seed_observation(index: usize, args: &Args) -> ObservationInput {
    let mut rng = XorShift::new(index as u64);
    let mut words = Vec::with_capacity(24);
    if rng.next() % 100 < u64::from(args.common_percent) {
        words.push(COMMON_WORD.to_string());
    }
    if index.is_multiple_of(2_000) {
        words.push(RARE_WORD.to_string());
    }
    for _ in 0..20 {
        words.push(format!("w{:04}", rng.next() % 5_000));
    }
    ObservationInput {
        id: None,
        kind: ObservationKind::Command,
        content: format!("seed {index}: {}", words.join(" ")),
        provenance: None,
        ts: None,
        scope_ids: scope(),
        redacted: None,
    }
}

fn observation(index: usize, tag: &str) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::ToolCall,
        content: format!("{tag} {index}: Edit src/lib.rs w0001 w0002"),
        provenance: None,
        ts: None,
        scope_ids: scope(),
        redacted: None,
    }
}

fn scope() -> ScopeIds {
    ScopeIds {
        session_id: Some("contention-session".into()),
        repo_id: Some(PROJECT_ROOT.into()),
        agent_id: None,
        user_id: None,
        task_id: None,
    }
}

fn broad_query() -> RetrieveOptions {
    query(COMMON_WORD)
}

fn small_query() -> RetrieveOptions {
    query(RARE_WORD)
}

fn query(text: &str) -> RetrieveOptions {
    RetrieveOptions {
        query: text.into(),
        scope_ids: ScopeIds {
            repo_id: Some(PROJECT_ROOT.into()),
            ..ScopeIds::default()
        },
        token_budget: None,
        max_candidates: Some(20),
        include_redacted: Some(false),
        query_mode: None,
    }
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}

/// Deterministic xorshift64* so the corpus is identical across runs.
struct XorShift(u64);

impl XorShift {
    fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
}
