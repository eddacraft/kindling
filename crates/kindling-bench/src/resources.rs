//! Low-overhead process resource sampling for repeatable workload profiles.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default)]
struct Snapshot {
    rss_kib: Option<u64>,
    threads: Option<u64>,
    fds: Option<u64>,
    process_jiffies: Option<u64>,
    total_jiffies: Option<u64>,
    /// Physical disk I/O from `/proc/.../io` (`read_bytes` / `write_bytes`).
    /// Often stays 0 when the page cache absorbs writes and no writeback
    /// has flushed yet (common on tmpfs and heavily cached local disks).
    read_bytes: Option<u64>,
    write_bytes: Option<u64>,
    /// Logical syscall I/O (`rchar` / `wchar`) — increments even when physical
    /// writeback has not run, so storage growth remains visible.
    rchar: Option<u64>,
    wchar: Option<u64>,
}

/// Resource usage observed while one workload runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSummary {
    pub platform: String,
    pub available: bool,
    /// `in-process` shares the harness process across groups; `isolated-child`
    /// runs each group in a fresh process so RSS/threads/FDs are per-group.
    pub measurement_scope: String,
    pub rss_start_mib: Option<f64>,
    pub rss_end_mib: Option<f64>,
    pub peak_rss_mib: Option<f64>,
    /// End minus start RSS for this workload window (may be negative).
    pub rss_growth_mib: Option<f64>,
    /// Peak RSS above the workload's starting baseline.
    pub peak_rss_over_start_mib: Option<f64>,
    pub threads_start: Option<u64>,
    pub peak_threads: Option<u64>,
    pub peak_threads_over_start: Option<u64>,
    pub file_descriptors_start: Option<u64>,
    pub peak_file_descriptors: Option<u64>,
    pub peak_file_descriptors_over_start: Option<u64>,
    /// Average CPU consumption where 1.0 is one fully saturated core.
    pub cpu_cores: Option<f64>,
    /// Physical disk bytes read (`/proc/self/io` `read_bytes`).
    pub read_bytes: Option<u64>,
    /// Physical disk bytes written (`/proc/self/io` `write_bytes`).
    pub write_bytes: Option<u64>,
    /// Logical read chars (`rchar`) — use when physical counters stay zero.
    pub logical_read_bytes: Option<u64>,
    /// Logical write chars (`wchar`) — use when physical counters stay zero.
    pub logical_write_bytes: Option<u64>,
}

/// Background sampler. On Linux it samples `/proc/self`; elsewhere resource
/// fields are reported as unavailable without failing the benchmark.
pub struct ResourceSampler {
    before: Snapshot,
    stop: Arc<AtomicBool>,
    handle: Option<JoinHandle<Snapshot>>,
    measurement_scope: &'static str,
}

impl ResourceSampler {
    #[must_use]
    pub fn start(interval: Duration) -> Self {
        Self::start_self(interval, "in-process")
    }

    /// Sample the current process but label the result as an isolated child
    /// (used inside a one-group child process).
    #[must_use]
    pub fn start_isolated_child(interval: Duration) -> Self {
        Self::start_self(interval, "isolated-child")
    }

    fn start_self(interval: Duration, measurement_scope: &'static str) -> Self {
        let before = snapshot();
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let handle = thread::spawn(move || {
            let mut peak = snapshot();
            while !thread_stop.load(Ordering::Relaxed) {
                thread::sleep(interval);
                let current = snapshot();
                peak.rss_kib = option_max(peak.rss_kib, current.rss_kib);
                peak.threads = option_max(peak.threads, current.threads);
                peak.fds = option_max(peak.fds, current.fds);
            }
            peak
        });
        Self {
            before,
            stop,
            handle: Some(handle),
            measurement_scope,
        }
    }

    #[must_use]
    pub fn finish(mut self) -> ResourceSummary {
        self.stop.store(true, Ordering::Relaxed);
        let peak = self
            .handle
            .take()
            .and_then(|handle| handle.join().ok())
            .unwrap_or_default();
        let after = snapshot();
        let ncpus = thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get);
        let cpu_cores = match (
            self.before.process_jiffies,
            after.process_jiffies,
            self.before.total_jiffies,
            after.total_jiffies,
        ) {
            (Some(proc_before), Some(proc_after), Some(total_before), Some(total_after)) => {
                Some(cpu_cores_for_window(
                    proc_after.saturating_sub(proc_before),
                    total_after.saturating_sub(total_before),
                    ncpus,
                ))
            }
            _ => None,
        };
        let peak_rss_kib = option_max(option_max(peak.rss_kib, self.before.rss_kib), after.rss_kib);
        let peak_threads = option_max(option_max(peak.threads, self.before.threads), after.threads);
        let peak_fds = option_max(option_max(peak.fds, self.before.fds), after.fds);
        let rss_start_mib = self.before.rss_kib.map(kib_to_mib);
        let rss_end_mib = after.rss_kib.map(kib_to_mib);
        let peak_rss_mib = peak_rss_kib.map(kib_to_mib);

        ResourceSummary {
            platform: std::env::consts::OS.to_string(),
            available: self.before.rss_kib.is_some(),
            measurement_scope: self.measurement_scope.to_string(),
            rss_start_mib,
            rss_end_mib,
            peak_rss_mib,
            rss_growth_mib: rss_end_mib
                .zip(rss_start_mib)
                .map(|(end, start)| end - start),
            peak_rss_over_start_mib: peak_rss_mib
                .zip(rss_start_mib)
                .map(|(peak, start)| (peak - start).max(0.0)),
            threads_start: self.before.threads,
            peak_threads,
            peak_threads_over_start: peak_threads
                .zip(self.before.threads)
                .map(|(peak, start)| peak.saturating_sub(start)),
            file_descriptors_start: self.before.fds,
            peak_file_descriptors: peak_fds,
            peak_file_descriptors_over_start: peak_fds
                .zip(self.before.fds)
                .map(|(peak, start)| peak.saturating_sub(start)),
            cpu_cores,
            read_bytes: delta(self.before.read_bytes, after.read_bytes),
            write_bytes: delta(self.before.write_bytes, after.write_bytes),
            logical_read_bytes: delta(self.before.rchar, after.rchar),
            logical_write_bytes: delta(self.before.wchar, after.wchar),
        }
    }
}

impl Drop for ResourceSampler {
    fn drop(&mut self) {
        // Cancellation-safe cleanup: error or early-return paths can drop a
        // sampler without calling `finish`. Signal the polling thread to exit
        // and join it so it does not keep running detached past the sampler's
        // lifetime.
        self.stop.store(true, Ordering::Relaxed);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn option_max(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (left, right) => left.or(right),
    }
}

fn delta(before: Option<u64>, after: Option<u64>) -> Option<u64> {
    Some(after?.saturating_sub(before?))
}

fn kib_to_mib(kib: u64) -> f64 {
    kib as f64 / 1024.0
}

/// Convert process and aggregate CPU deltas into saturated-core units.
#[must_use]
pub fn cpu_cores_for_window(process_delta: u64, total_delta: u64, ncpus: usize) -> f64 {
    if total_delta == 0 {
        return 0.0;
    }
    process_delta as f64 * ncpus as f64 / total_delta as f64
}

/// Parse resident memory and thread count from `/proc/<pid>/status`.
#[must_use]
pub fn parse_proc_status(status: &str) -> Option<(u64, u64)> {
    let mut rss_kib = None;
    let mut threads = None;
    for line in status.lines() {
        if let Some(value) = line.strip_prefix("VmRSS:") {
            rss_kib = value.split_whitespace().next()?.parse().ok();
        } else if let Some(value) = line.strip_prefix("Threads:") {
            threads = value.trim().parse().ok();
        }
    }
    Some((rss_kib?, threads?))
}

/// Parse user + system CPU jiffies from `/proc/<pid>/stat`.
#[must_use]
pub fn parse_proc_stat(stat: &str) -> Option<u64> {
    let after_comm = stat.rfind(") ").and_then(|index| stat.get(index + 2..))?;
    let fields = after_comm.split_whitespace().collect::<Vec<_>>();
    let user = fields.get(11)?.parse::<u64>().ok()?;
    let system = fields.get(12)?.parse::<u64>().ok()?;
    Some(user.saturating_add(system))
}

/// Parsed `/proc/<pid>/io` counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProcIo {
    pub read_bytes: u64,
    pub write_bytes: u64,
    pub rchar: u64,
    pub wchar: u64,
}

/// Parse physical and logical I/O counters from `/proc/<pid>/io`.
///
/// Physical `read_bytes`/`write_bytes` only advance when the kernel accounts
/// storage I/O. Logical `rchar`/`wchar` advance on syscall traffic and remain
/// useful when page cache or tmpfs leaves physical counters at zero.
#[must_use]
pub fn parse_proc_io(io: &str) -> Option<ProcIo> {
    let mut counters = ProcIo::default();
    let mut saw_physical = false;
    let mut saw_logical = false;
    for line in io.lines() {
        if let Some(value) = line.strip_prefix("read_bytes:") {
            counters.read_bytes = value.trim().parse().ok()?;
            saw_physical = true;
        } else if let Some(value) = line.strip_prefix("write_bytes:") {
            counters.write_bytes = value.trim().parse().ok()?;
            saw_physical = true;
        } else if let Some(value) = line.strip_prefix("rchar:") {
            counters.rchar = value.trim().parse().ok()?;
            saw_logical = true;
        } else if let Some(value) = line.strip_prefix("wchar:") {
            counters.wchar = value.trim().parse().ok()?;
            saw_logical = true;
        }
    }
    (saw_physical || saw_logical).then_some(counters)
}

#[cfg(target_os = "linux")]
fn snapshot() -> Snapshot {
    let (rss_kib, threads) = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|value| parse_proc_status(&value))
        .map_or((None, None), |(rss, threads)| (Some(rss), Some(threads)));
    let process_jiffies = std::fs::read_to_string("/proc/self/stat")
        .ok()
        .and_then(|value| parse_proc_stat(&value));
    let total_jiffies = std::fs::read_to_string("/proc/stat")
        .ok()
        .and_then(|value| parse_total_cpu_jiffies(&value));
    let io = std::fs::read_to_string("/proc/self/io")
        .ok()
        .and_then(|value| parse_proc_io(&value));
    let fds = std::fs::read_dir("/proc/self/fd")
        .ok()
        .map(|entries| entries.filter_map(Result::ok).count() as u64);
    Snapshot {
        rss_kib,
        threads,
        fds,
        process_jiffies,
        total_jiffies,
        read_bytes: io.map(|c| c.read_bytes),
        write_bytes: io.map(|c| c.write_bytes),
        rchar: io.map(|c| c.rchar),
        wchar: io.map(|c| c.wchar),
    }
}

#[cfg(not(target_os = "linux"))]
fn snapshot() -> Snapshot {
    Snapshot::default()
}

fn parse_total_cpu_jiffies(stat: &str) -> Option<u64> {
    let mut fields = stat.lines().next()?.split_whitespace();
    (fields.next()? == "cpu").then_some(())?;
    fields
        .take(8)
        .map(str::parse::<u64>)
        .try_fold(0u64, |total, value| Some(total.saturating_add(value.ok()?)))
}
