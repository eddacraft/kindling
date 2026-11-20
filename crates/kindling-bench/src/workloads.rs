//! End-to-end workload profiles over the in-process service and bundled daemon.

use std::error::Error;
use std::io;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use kindling_client::{Client, ClientConfig, Spawner, Transport, EXPECTED_SCHEMA_VERSION};
use kindling_runtime::{Runtime, RuntimeConfig, SpawnStrategy};
use kindling_service::{AppendObservationOptions, KindlingService};
use kindling_types::{
    ListObservationsRequest, ObservationInput, ObservationKind, RetrieveOptions, ScopeIds,
};
use serde::{Deserialize, Serialize};

use crate::profile::Profile;
use crate::resources::{ResourceSampler, ResourceSummary};
use crate::statistics::LatencySummary;

type BoxError = Box<dyn Error + Send + Sync>;

const PROJECT_ROOT: &str = "benchmark-project";
const SESSION_ID: &str = "benchmark-session";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub schema_version: u32,
    pub generated_at_epoch_ms: u128,
    pub profile: Profile,
    pub system: SystemInfo,
    pub groups: Vec<WorkloadGroup>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfo {
    pub os: String,
    pub architecture: String,
    pub logical_cpus: usize,
    pub release_build: bool,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadGroup {
    pub name: String,
    pub description: String,
    pub metrics: Vec<WorkloadMetric>,
    pub resources: ResourceSummary,
    pub storage_bytes: u64,
    pub spool_bytes: u64,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkloadMetric {
    pub name: String,
    pub operation_count: usize,
    pub rows_processed: Option<usize>,
    pub latency: LatencySummary,
}

/// How resource measurements are attributed across groups.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MeasurementMode {
    /// All groups share one process (historical default; RSS is cumulative).
    #[default]
    InProcess,
    /// Each group runs in a fresh child process so RSS/threads/FDs are per-group.
    IsolatedChild,
}

const GROUP_NAMES: &[&str] = &["cold-start", "direct-service", "daemon", "outage-recovery"];

pub async fn run(profile: Profile) -> Result<BenchmarkReport, BoxError> {
    run_with_mode(profile, MeasurementMode::InProcess).await
}

pub async fn run_with_mode(
    profile: Profile,
    mode: MeasurementMode,
) -> Result<BenchmarkReport, BoxError> {
    let groups = match mode {
        MeasurementMode::InProcess => {
            vec![
                cold_start_group(&profile, MeasurementMode::InProcess).await?,
                direct_service_group(&profile, MeasurementMode::InProcess)?,
                daemon_group(&profile, MeasurementMode::InProcess).await?,
                spool_group(&profile, MeasurementMode::InProcess).await?,
            ]
        }
        MeasurementMode::IsolatedChild => run_groups_isolated(&profile)?,
    };
    Ok(BenchmarkReport {
        schema_version: 1,
        generated_at_epoch_ms: SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis(),
        profile,
        system: SystemInfo {
            os: std::env::consts::OS.to_string(),
            architecture: std::env::consts::ARCH.to_string(),
            logical_cpus: std::thread::available_parallelism()
                .map_or(1, std::num::NonZeroUsize::get),
            release_build: !cfg!(debug_assertions),
        },
        groups,
    })
}

/// Run a single named group in the current process (child entrypoint).
pub async fn run_group(
    profile: Profile,
    group: &str,
    mode: MeasurementMode,
) -> Result<WorkloadGroup, BoxError> {
    match group {
        "cold-start" => cold_start_group(&profile, mode).await,
        "direct-service" => direct_service_group(&profile, mode),
        "daemon" => daemon_group(&profile, mode).await,
        "outage-recovery" => spool_group(&profile, mode).await,
        other => Err(format!(
            "unknown group '{other}'; expected one of {}",
            GROUP_NAMES.join(", ")
        )
        .into()),
    }
}

fn run_groups_isolated(profile: &Profile) -> Result<Vec<WorkloadGroup>, BoxError> {
    let exe = std::env::current_exe()?;
    // Pass the full profile (not just its name) so a customized profile is
    // honoured by each child instead of being reconstructed as a preset.
    let profile_json = serde_json::to_string(profile)?;
    let mut groups = Vec::with_capacity(GROUP_NAMES.len());
    for group in GROUP_NAMES {
        let output = Command::new(&exe)
            .args([
                "--profile-json",
                &profile_json,
                "--only-group",
                group,
                "--measurement-scope",
                "isolated-child",
            ])
            .output()?;
        if !output.status.success() {
            return Err(format!(
                "isolated group '{group}' failed (status {}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            )
            .into());
        }
        let group: WorkloadGroup = serde_json::from_slice(&output.stdout).map_err(|err| {
            format!(
                "failed to parse isolated group '{group}' stdout: {err}; body={}",
                String::from_utf8_lossy(&output.stdout)
            )
        })?;
        groups.push(group);
    }
    Ok(groups)
}

fn resource_sampler(mode: MeasurementMode) -> ResourceSampler {
    match mode {
        MeasurementMode::InProcess => ResourceSampler::start(Duration::from_millis(2)),
        MeasurementMode::IsolatedChild => {
            ResourceSampler::start_isolated_child(Duration::from_millis(2))
        }
    }
}

async fn cold_start_group(
    profile: &Profile,
    mode: MeasurementMode,
) -> Result<WorkloadGroup, BoxError> {
    let sampler = resource_sampler(mode);
    let mut samples = Vec::with_capacity(profile.cold_starts);
    let elapsed = Instant::now();
    let mut storage_bytes = 0u64;
    for _ in 0..profile.cold_starts {
        let temp = tempfile::tempdir()?;
        let started = Instant::now();
        let runtime = Runtime::start(RuntimeConfig::with_home(
            temp.path(),
            PROJECT_ROOT,
            SpawnStrategy::Embedded,
        ))
        .await?;
        samples.push(started.elapsed());
        runtime.shutdown().await?;
        storage_bytes = storage_bytes.saturating_add(directory_size(temp.path())?);
    }
    let elapsed = elapsed.elapsed();
    Ok(WorkloadGroup {
        name: "cold-start".into(),
        description: "Fresh bundled daemon startup through a health-ready runtime".into(),
        metrics: vec![metric("runtime-start", samples, elapsed, None)],
        resources: sampler.finish(),
        storage_bytes,
        spool_bytes: 0,
    })
}

fn direct_service_group(
    profile: &Profile,
    mode: MeasurementMode,
) -> Result<WorkloadGroup, BoxError> {
    let temp = tempfile::tempdir()?;
    let service = KindlingService::open(&temp.path().join("kindling.db"))?;
    for index in 0..profile.warmup_operations {
        service.append_observation(input(index), AppendObservationOptions::default())?;
    }

    let sampler = resource_sampler(mode);
    let write_start = Instant::now();
    let mut write_samples = Vec::with_capacity(profile.write_operations);
    for index in 0..profile.write_operations {
        let started = Instant::now();
        service.append_observation(
            input(profile.warmup_operations + index),
            AppendObservationOptions::default(),
        )?;
        write_samples.push(started.elapsed());
    }
    let write_elapsed = write_start.elapsed();

    seed_service(&service, profile.seed_observations)?;
    let (page_samples, page_elapsed, page_rows) = measure_sync(profile.read_repetitions, || {
        let page = service.list_observations(list_request(profile.page_limit, None))?;
        Ok(page.observations.len())
    })?;
    let (scan_samples, scan_elapsed, scan_rows) = measure_sync(profile.read_repetitions, || {
        list_all_service(&service, profile.page_limit)
    })?;
    let (retrieve_samples, retrieve_elapsed, retrieve_rows) =
        measure_sync(profile.read_repetitions, || {
            let result = service.retrieve(retrieve_options())?;
            Ok(result.candidates.len())
        })?;

    Ok(WorkloadGroup {
        name: "direct-service".into(),
        description: "SQLite/service baseline without daemon transport".into(),
        metrics: vec![
            metric("append", write_samples, write_elapsed, None),
            metric("list-page", page_samples, page_elapsed, Some(page_rows)),
            metric(
                "list-full-scan",
                scan_samples,
                scan_elapsed,
                Some(scan_rows),
            ),
            metric(
                "ranked-retrieve",
                retrieve_samples,
                retrieve_elapsed,
                Some(retrieve_rows),
            ),
        ],
        resources: sampler.finish(),
        storage_bytes: directory_size(temp.path())?,
        spool_bytes: 0,
    })
}

async fn daemon_group(profile: &Profile, mode: MeasurementMode) -> Result<WorkloadGroup, BoxError> {
    let temp = tempfile::tempdir()?;
    let runtime = Runtime::start(RuntimeConfig::with_home(
        temp.path(),
        PROJECT_ROOT,
        SpawnStrategy::Embedded,
    ))
    .await?;
    for index in 0..profile.warmup_operations {
        runtime
            .spooled_client()
            .append_observation(input(index), None, None)
            .await?;
    }

    let sampler = resource_sampler(mode);
    let write_start = Instant::now();
    let mut write_samples = Vec::with_capacity(profile.write_operations);
    for index in 0..profile.write_operations {
        let started = Instant::now();
        runtime
            .spooled_client()
            .append_observation(input(profile.warmup_operations + index), None, None)
            .await?;
        write_samples.push(started.elapsed());
    }
    let write_elapsed = write_start.elapsed();

    let concurrent_start = Instant::now();
    let concurrent_samples = concurrent_appends(
        runtime.client().clone(),
        profile.concurrent_writers,
        profile.writes_per_task,
    )
    .await?;
    let concurrent_elapsed = concurrent_start.elapsed();

    seed_client(runtime.client(), profile.seed_observations).await?;
    let (page_samples, page_elapsed, page_rows) = measure_async(profile.read_repetitions, || {
        let client = runtime.client().clone();
        async move {
            let page = client
                .list_observations(list_request(profile.page_limit, None))
                .await?;
            Ok::<usize, kindling_client::ClientError>(page.observations.len())
        }
    })
    .await?;
    let (scan_samples, scan_elapsed, scan_rows) = measure_async(profile.read_repetitions, || {
        let client = runtime.client().clone();
        async move { list_all_client(&client, profile.page_limit).await }
    })
    .await?;
    let (retrieve_samples, retrieve_elapsed, retrieve_rows) =
        measure_async(profile.read_repetitions, || {
            let client = runtime.client().clone();
            async move {
                let result = client.retrieve(retrieve_options()).await?;
                Ok::<usize, kindling_client::ClientError>(result.candidates.len())
            }
        })
        .await?;

    let resources = sampler.finish();
    let storage_bytes = directory_size(temp.path())?;
    runtime.shutdown().await?;
    Ok(WorkloadGroup {
        name: "daemon".into(),
        description: "Bundled daemon round trips, exhaustive reads, and concurrent writers".into(),
        metrics: vec![
            metric("spooled-append-warm", write_samples, write_elapsed, None),
            metric(
                "append-concurrent",
                concurrent_samples,
                concurrent_elapsed,
                None,
            ),
            metric("list-page", page_samples, page_elapsed, Some(page_rows)),
            metric(
                "list-full-scan",
                scan_samples,
                scan_elapsed,
                Some(scan_rows),
            ),
            metric(
                "ranked-retrieve",
                retrieve_samples,
                retrieve_elapsed,
                Some(retrieve_rows),
            ),
        ],
        resources,
        storage_bytes,
        spool_bytes: file_size(&temp.path().join("spool.ndjson")),
    })
}

async fn spool_group(profile: &Profile, mode: MeasurementMode) -> Result<WorkloadGroup, BoxError> {
    let temp = tempfile::tempdir()?;
    let spool_path = temp.path().join("spool.ndjson");
    let down_client = Client::with_config(ClientConfig {
        socket_path: temp.path().join("missing.sock"),
        port_path: temp.path().join("missing.port"),
        project_root: PROJECT_ROOT.into(),
        expected_schema_version: EXPECTED_SCHEMA_VERSION,
        connect_timeout: Duration::from_millis(5),
        poll_interval: Duration::from_millis(1),
        spawn: Spawner::custom(|| {
            Err(io::Error::new(
                io::ErrorKind::NotFound,
                "benchmark daemon intentionally unavailable",
            ))
        }),
        transport: Transport::default(),
        spawn_log_path: None,
    });
    let spooled = kindling_client::spool::SpooledClient::new(down_client, spool_path.clone());
    let sampler = resource_sampler(mode);
    let spool_start = Instant::now();
    let mut spool_samples = Vec::with_capacity(profile.spool_operations);
    for index in 0..profile.spool_operations {
        let started = Instant::now();
        spooled.append_observation(input(index), None, None).await?;
        spool_samples.push(started.elapsed());
    }
    let spool_elapsed = spool_start.elapsed();
    let window_size = (spool_samples.len() / 10).max(1);
    let early_samples = spool_samples[..window_size].to_vec();
    let late_samples = spool_samples[spool_samples.len() - window_size..].to_vec();
    let early_elapsed: Duration = early_samples.iter().copied().sum();
    let late_elapsed: Duration = late_samples.iter().copied().sum();
    let spool_bytes = file_size(&spool_path);
    drop(spooled);

    let mut runtime_config =
        RuntimeConfig::with_home(temp.path(), PROJECT_ROOT, SpawnStrategy::Embedded);
    runtime_config.spool_path = Some(spool_path.clone());
    let runtime = Runtime::start(runtime_config).await?;
    let flush_start = Instant::now();
    let flush_report = runtime.spooled_client().flush().await?;
    let flush_elapsed = flush_start.elapsed();
    let flush_samples = [flush_elapsed];
    let resources = sampler.finish();
    let storage_bytes = directory_size(temp.path())?;
    runtime.shutdown().await?;

    Ok(WorkloadGroup {
        name: "outage-recovery".into(),
        description: "Unavailable-daemon buffering followed by durable replay".into(),
        metrics: vec![
            metric("spool-append", spool_samples, spool_elapsed, None),
            metric("spool-append-early", early_samples, early_elapsed, None),
            metric("spool-append-late", late_samples, late_elapsed, None),
            WorkloadMetric {
                name: "spool-replay".into(),
                operation_count: flush_report.replayed,
                rows_processed: Some(flush_report.replayed),
                latency: LatencySummary::from_samples_with_operations(
                    &flush_samples,
                    flush_elapsed,
                    flush_report.replayed,
                ),
            },
        ],
        resources,
        storage_bytes,
        spool_bytes,
    })
}

fn metric(
    name: &str,
    samples: Vec<Duration>,
    elapsed: Duration,
    rows_processed: Option<usize>,
) -> WorkloadMetric {
    WorkloadMetric {
        name: name.into(),
        operation_count: samples.len(),
        rows_processed,
        latency: LatencySummary::from_samples(&samples, elapsed),
    }
}

fn input(index: usize) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Command,
        content: format!(
            "command invocation {index}: validate authentication cache and feature flags"
        ),
        provenance: None,
        ts: None,
        scope_ids: benchmark_scope(),
        redacted: None,
    }
}

fn benchmark_scope() -> ScopeIds {
    ScopeIds {
        session_id: Some(SESSION_ID.into()),
        repo_id: Some(PROJECT_ROOT.into()),
        agent_id: Some("benchmark-agent".into()),
        user_id: None,
        task_id: None,
    }
}

fn list_request(limit: u32, cursor: Option<String>) -> ListObservationsRequest {
    ListObservationsRequest {
        scope_ids: benchmark_scope(),
        kinds: vec![ObservationKind::Command],
        since: None,
        until: None,
        limit: Some(limit),
        cursor,
        include_redacted: Some(false),
    }
}

fn retrieve_options() -> RetrieveOptions {
    RetrieveOptions {
        query: "authentication cache".into(),
        scope_ids: benchmark_scope(),
        token_budget: None,
        max_candidates: Some(20),
        include_redacted: Some(false),
    }
}

fn seed_service(service: &KindlingService, count: usize) -> Result<(), BoxError> {
    for index in 0..count {
        service.append_observation(input(1_000_000 + index), Default::default())?;
    }
    Ok(())
}

async fn seed_client(client: &Client, count: usize) -> Result<(), BoxError> {
    for index in 0..count {
        client
            .append_observation(input(2_000_000 + index), None, None)
            .await?;
    }
    Ok(())
}

fn list_all_service(service: &KindlingService, limit: u32) -> Result<usize, BoxError> {
    let mut cursor = None;
    let mut rows = 0usize;
    loop {
        let page = service.list_observations(list_request(limit, cursor.take()))?;
        rows += page.observations.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(rows);
        }
    }
}

async fn list_all_client(
    client: &Client,
    limit: u32,
) -> Result<usize, kindling_client::ClientError> {
    let mut cursor = None;
    let mut rows = 0usize;
    loop {
        let page = client
            .list_observations(list_request(limit, cursor.take()))
            .await?;
        rows += page.observations.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            return Ok(rows);
        }
    }
}

async fn concurrent_appends(
    client: Client,
    writers: usize,
    writes_per_task: usize,
) -> Result<Vec<Duration>, BoxError> {
    let barrier = std::sync::Arc::new(tokio::sync::Barrier::new(writers));
    let mut tasks = Vec::with_capacity(writers);
    for writer in 0..writers {
        let client = client.clone();
        let barrier = barrier.clone();
        tasks.push(tokio::spawn(async move {
            barrier.wait().await;
            let mut samples = Vec::with_capacity(writes_per_task);
            for index in 0..writes_per_task {
                let started = Instant::now();
                client
                    .append_observation(
                        input(3_000_000 + writer * writes_per_task + index),
                        None,
                        None,
                    )
                    .await?;
                samples.push(started.elapsed());
            }
            Ok::<Vec<Duration>, kindling_client::ClientError>(samples)
        }));
    }
    let mut samples = Vec::with_capacity(writers * writes_per_task);
    for task in tasks {
        samples.extend(task.await??);
    }
    Ok(samples)
}

fn measure_sync<F>(
    repetitions: usize,
    mut operation: F,
) -> Result<(Vec<Duration>, Duration, usize), BoxError>
where
    F: FnMut() -> Result<usize, BoxError>,
{
    let elapsed = Instant::now();
    let mut samples = Vec::with_capacity(repetitions);
    let mut rows = 0usize;
    for _ in 0..repetitions {
        let started = Instant::now();
        rows = rows.saturating_add(operation()?);
        samples.push(started.elapsed());
    }
    Ok((samples, elapsed.elapsed(), rows))
}

async fn measure_async<F, Fut, E>(
    repetitions: usize,
    mut operation: F,
) -> Result<(Vec<Duration>, Duration, usize), BoxError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<usize, E>>,
    E: Error + Send + Sync + 'static,
{
    let elapsed = Instant::now();
    let mut samples = Vec::with_capacity(repetitions);
    let mut rows = 0usize;
    for _ in 0..repetitions {
        let started = Instant::now();
        rows = rows.saturating_add(operation().await?);
        samples.push(started.elapsed());
    }
    Ok((samples, elapsed.elapsed(), rows))
}

fn directory_size(path: &Path) -> io::Result<u64> {
    let mut bytes = 0u64;
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            bytes = bytes.saturating_add(directory_size(&entry.path())?);
        } else {
            bytes = bytes.saturating_add(metadata.len());
        }
    }
    Ok(bytes)
}

fn file_size(path: &Path) -> u64 {
    std::fs::metadata(path).map_or(0, |metadata| metadata.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn smoke_report_exposes_early_and_late_outage_windows() {
        let report = run(Profile::from_name("smoke").expect("smoke profile"))
            .await
            .expect("smoke benchmark");
        let outage = report
            .groups
            .iter()
            .find(|group| group.name == "outage-recovery")
            .expect("outage group");
        let names: Vec<&str> = outage
            .metrics
            .iter()
            .map(|metric| metric.name.as_str())
            .collect();
        assert!(names.contains(&"spool-append-early"), "{names:?}");
        assert!(names.contains(&"spool-append-late"), "{names:?}");
    }
}
