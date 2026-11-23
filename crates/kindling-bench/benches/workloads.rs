use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use kindling_client::{Client, ClientConfig, Spawner, Transport, EXPECTED_SCHEMA_VERSION};
use kindling_runtime::{Runtime, RuntimeConfig, SpawnStrategy};
use kindling_service::{AppendObservationOptions, KindlingService};
use kindling_types::{
    ListObservationsRequest, ObservationInput, ObservationKind, RetrieveOptions, ScopeIds,
};

const PROJECT_ROOT: &str = "benchmark-project";

fn input(index: usize) -> ObservationInput {
    ObservationInput {
        id: None,
        kind: ObservationKind::Command,
        content: format!("command invocation {index}: authentication cache and feature flags"),
        provenance: None,
        ts: None,
        scope_ids: scope(),
        redacted: None,
    }
}

fn deduplicated_input() -> ObservationInput {
    let mut input = input(usize::MAX);
    input.id = Some("benchmark-deduplicated-observation".into());
    input
}

fn scope() -> ScopeIds {
    ScopeIds {
        session_id: Some("benchmark-session".into()),
        repo_id: Some(PROJECT_ROOT.into()),
        agent_id: Some("benchmark-agent".into()),
        user_id: None,
        task_id: None,
    }
}

fn list_request(cursor: Option<String>) -> ListObservationsRequest {
    ListObservationsRequest {
        scope_ids: scope(),
        kinds: vec![ObservationKind::Command],
        since: None,
        until: None,
        limit: Some(500),
        cursor,
        include_redacted: Some(false),
    }
}

fn retrieve_options() -> RetrieveOptions {
    RetrieveOptions {
        query: "authentication cache".into(),
        scope_ids: scope(),
        token_budget: None,
        max_candidates: Some(20),
        include_redacted: Some(false),
        query_mode: None,
    }
}

fn seed_service(service: &KindlingService, count: usize) {
    for index in 0..count {
        service
            .append_observation(input(index), AppendObservationOptions::default())
            .expect("seed direct service");
    }
}

fn list_all_service(service: &KindlingService) -> usize {
    let mut cursor = None;
    let mut rows = 0;
    loop {
        let page = service
            .list_observations(list_request(cursor.take()))
            .expect("list direct service");
        rows += page.observations.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            return rows;
        }
    }
}

async fn seed_client(client: &Client, count: usize) {
    for index in 0..count {
        client
            .append_observation(input(index), None, None)
            .await
            .expect("seed daemon");
    }
}

async fn list_all_client(client: &Client) -> usize {
    let mut cursor = None;
    let mut rows = 0;
    loop {
        let page = client
            .list_observations(list_request(cursor.take()))
            .await
            .expect("list daemon");
        rows += page.observations.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            return rows;
        }
    }
}

fn direct_service(c: &mut Criterion) {
    let write_temp = tempfile::tempdir().expect("write tempdir");
    let write_service =
        KindlingService::open(&write_temp.path().join("kindling.db")).expect("write service");
    let read_temp = tempfile::tempdir().expect("read tempdir");
    let read_service =
        KindlingService::open(&read_temp.path().join("kindling.db")).expect("read service");
    seed_service(&read_service, 10_000);
    let next = AtomicUsize::new(10_000);
    write_service
        .append_observation(deduplicated_input(), Default::default())
        .expect("seed deduplicated append");

    let mut group = c.benchmark_group("direct-service");
    group.throughput(Throughput::Elements(1));
    group.bench_function("append", |b| {
        b.iter(|| {
            let index = next.fetch_add(1, Ordering::Relaxed);
            write_service
                .append_observation(input(index), Default::default())
                .expect("append")
        });
    });
    group.bench_function("append-deduplicated", |b| {
        b.iter(|| {
            write_service
                .append_observation(deduplicated_input(), Default::default())
                .expect("deduplicated append")
        });
    });
    group.bench_function("list-page-500", |b| {
        b.iter(|| {
            read_service
                .list_observations(list_request(None))
                .expect("list page")
        });
    });
    group.bench_function("list-full-scan-10k", |b| {
        b.iter(|| std::hint::black_box(list_all_service(&read_service)));
    });
    group.bench_function("ranked-retrieve-10k", |b| {
        b.iter(|| read_service.retrieve(retrieve_options()).expect("retrieve"));
    });
    group.finish();
}

fn daemon(c: &mut Criterion) {
    let tokio = tokio::runtime::Runtime::new().expect("tokio runtime");
    let write_temp = tempfile::tempdir().expect("write tempdir");
    let write_runtime = tokio
        .block_on(Runtime::start(RuntimeConfig::with_home(
            write_temp.path(),
            PROJECT_ROOT,
            SpawnStrategy::Embedded,
        )))
        .expect("write daemon");
    let read_temp = tempfile::tempdir().expect("read tempdir");
    let read_runtime = tokio
        .block_on(Runtime::start(RuntimeConfig::with_home(
            read_temp.path(),
            PROJECT_ROOT,
            SpawnStrategy::Embedded,
        )))
        .expect("read daemon");
    tokio.block_on(seed_client(read_runtime.client(), 10_000));
    tokio
        .block_on(
            write_runtime
                .client()
                .append_observation(deduplicated_input(), None, None),
        )
        .expect("seed deduplicated append");
    let next = AtomicUsize::new(10_000);

    let mut group = c.benchmark_group("daemon");
    group.throughput(Throughput::Elements(1));
    group.bench_function("spooled-append-warm", |b| {
        b.to_async(&tokio).iter(|| async {
            let index = next.fetch_add(1, Ordering::Relaxed);
            write_runtime
                .spooled_client()
                .append_observation(input(index), None, None)
                .await
                .expect("append")
        });
    });
    group.bench_function("spooled-append-deduplicated", |b| {
        b.to_async(&tokio).iter(|| async {
            write_runtime
                .spooled_client()
                .append_observation(deduplicated_input(), None, None)
                .await
                .expect("deduplicated append")
        });
    });
    group.bench_function("list-page-500", |b| {
        b.to_async(&tokio).iter(|| async {
            read_runtime
                .client()
                .list_observations(list_request(None))
                .await
                .expect("list page")
        });
    });
    group.bench_function("list-full-scan-10k", |b| {
        b.to_async(&tokio)
            .iter(|| async { std::hint::black_box(list_all_client(read_runtime.client()).await) });
    });
    group.bench_function("ranked-retrieve-10k", |b| {
        b.to_async(&tokio).iter(|| async {
            read_runtime
                .client()
                .retrieve(retrieve_options())
                .await
                .expect("retrieve")
        });
    });
    group.finish();
    tokio
        .block_on(write_runtime.shutdown())
        .expect("write shutdown");
    tokio
        .block_on(read_runtime.shutdown())
        .expect("read shutdown");
}

fn outage_buffer(c: &mut Criterion) {
    let tokio = tokio::runtime::Runtime::new().expect("tokio runtime");
    let temp = tempfile::tempdir().expect("tempdir");
    let down_client = |name: &str| {
        Client::with_config(ClientConfig {
            socket_path: temp.path().join(format!("missing-{name}.sock")),
            port_path: temp.path().join(format!("missing-{name}.port")),
            project_root: PROJECT_ROOT.into(),
            expected_schema_version: EXPECTED_SCHEMA_VERSION,
            connect_timeout: Duration::from_millis(5),
            poll_interval: Duration::from_millis(1),
            spawn: Spawner::custom(|| {
                Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "benchmark daemon intentionally unavailable",
                ))
            }),
            transport: Transport::default(),
            spawn_log_path: None,
        })
    };
    let spooled = kindling_client::spool::SpooledClient::new(
        down_client("unbounded"),
        temp.path().join("spool.ndjson"),
    );
    let next = AtomicUsize::new(0);
    c.bench_function("outage-recovery/spool-append", |b| {
        b.to_async(&tokio).iter(|| async {
            let index = next.fetch_add(1, Ordering::Relaxed);
            spooled
                .append_observation(input(index), None, None)
                .await
                .expect("spool append")
        });
    });

    let capped = kindling_client::spool::SpooledClient::with_config(
        down_client("capped"),
        kindling_client::spool::SpoolConfig::new(temp.path().join("spool-capped.ndjson"))
            .with_max_bytes(64 * 1024 * 1024),
    );
    let capped_next = AtomicUsize::new(0);
    c.bench_function("outage-recovery/spool-append-capped", |b| {
        b.to_async(&tokio).iter(|| async {
            let index = capped_next.fetch_add(1, Ordering::Relaxed);
            capped
                .append_observation(input(index), None, None)
                .await
                .expect("capped spool append")
        });
    });
}

fn cold_start(c: &mut Criterion) {
    let tokio = tokio::runtime::Runtime::new().expect("tokio runtime");
    c.bench_function("cold-start/runtime-start", |b| {
        b.iter(|| {
            let temp = tempfile::tempdir().expect("tempdir");
            tokio.block_on(async {
                let runtime = Runtime::start(RuntimeConfig::with_home(
                    temp.path(),
                    PROJECT_ROOT,
                    SpawnStrategy::Embedded,
                ))
                .await
                .expect("start");
                runtime.shutdown().await.expect("shutdown");
            });
        });
    });
}

fn criterion_config() -> Criterion {
    Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_secs(1))
        .measurement_time(Duration::from_secs(3))
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = direct_service, daemon, outage_buffer, cold_start
}
criterion_main!(benches);
