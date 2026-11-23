# kindling benchmarks

This crate measures the public workloads that can affect an integration using
kindling as a bundled local service. It is private (`publish = false`) and adds
no dependencies to the shipped client, runtime, or daemon.

The suite covers:

- fresh bundled-daemon startup;
- direct SQLite/service writes as the no-transport baseline;
- warm and deduplicated daemon writes through the durable spool wrapper;
- concurrent daemon writers;
- one-page and exhaustive keyset-paginated reads;
- ranked full-text retrieval;
- unavailable-daemon spool writes and replay;
- process CPU, RSS, threads, file descriptors, physical I/O counters, database
  growth, and spool growth.

Benchmark names describe kindling behavior rather than any downstream product.

## Statistical benchmarks

Run the Criterion suite in release mode:

```bash
cargo bench -p kindling-bench --bench workloads
```

Filter to one workload when iterating:

```bash
cargo bench -p kindling-bench --bench workloads -- daemon/list-full-scan
```

Criterion writes its normal comparison data and reports under
`target/criterion/`. Do not commit those machine-specific results.

## Workload and resource profile

The profiler emits stable-schema JSON suitable for archiving or comparing in
CI and lab runs:

```bash
cargo run -p kindling-bench --release -- --profile smoke --pretty
cargo run -p kindling-bench --release -- --profile standard > kindling-perf.json
cargo run -p kindling-bench --release -- --profile stress > kindling-stress.json

# Per-group resource isolation (recommended for release RSS/thread/FD gates):
cargo run -p kindling-bench --release -- --profile standard --isolated-process \
  > kindling-perf-isolated.json
```

Do **not** commit `kindling-perf.json` / `kindling-stress.json` or Criterion
trees here. Consumer-side historical tables live in the **anvil** repo under
`benchmarks/history/kindling/`.

| Profile    | Purpose                          | Seed rows | Measured writes | Concurrent writes | Spool/replay rows |
| ---------- | -------------------------------- | --------: | --------------: | ----------------: | ----------------: |
| `smoke`    | Fast correctness and local check |     2,000 |             200 |               200 |               100 |
| `standard` | Representative workstation run   |    20,000 |           2,000 |             4,000 |             1,000 |
| `stress`   | Scaling and resource pressure    |   200,000 |          20,000 |            32,000 |           100,000 |

Latency is reported in microseconds with min, mean, p50, p95, p99, max, and
operations/second. Full-scan metrics also report rows processed so pagination
completeness is visible. Outage writes include aggregate, first-10%, and
last-10% windows so backlog-dependent latency is not hidden by one percentile.

Resource sampling uses `/proc/self` on Linux. Default mode is **in-process**:
groups share one process, so RSS/thread peaks can accumulate across groups
(directional only). Prefer **`--isolated-process`** when collecting numbers for
release resource gates: each group runs in a fresh child so deltas are
per-workload.

Each group reports:

- average CPU in saturated-core units (`1.0` = one fully used core), normalised
  by the online CPUs in `/proc/stat`, so it stays correct under an affinity
  mask or CPU quota;
- start/end/peak RSS and peak-over-start;
- threads and file descriptors with peak-over-start deltas;
- **physical** `read_bytes` / `write_bytes` (kernel disk accounting — often `0`
  on tmpfs or when writeback has not flushed);
- **logical** `logicalReadBytes` / `logicalWriteBytes` (`rchar` / `wchar`) which
  still advance when physical counters stay flat. The sampler's own `/proc`
  reads are counted and subtracted exactly, so an idle window reports `0`.

If the background resource sampler fails, the benchmark returns an error instead
of emitting a successful report with fabricated default resource measurements.
Successful reports retain the schema described above.

Unsupported operating systems return `available: false` and `null` resource
fields while still producing latency, throughput, and storage measurements.

For comparable numbers, use the same release build, hardware, filesystem,
power profile, dataset profile, and background-load conditions. Run the suite
more than once and use Criterion's comparison rather than treating a single
profiler run as a regression threshold.
