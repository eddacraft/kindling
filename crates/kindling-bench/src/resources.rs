//! Low-overhead process resource sampling for repeatable workload profiles.

use std::any::Any;
use std::fmt;
use std::sync::mpsc::{self, Sender};
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
    /// CPUs covered by `total_jiffies` (`cpuN` lines in the same `/proc/stat`).
    online_cpus: Option<usize>,
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

#[derive(Debug, PartialEq, Eq)]
pub enum SamplerError {
    WorkerPanicked(String),
}

impl fmt::Display for SamplerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WorkerPanicked(message) => {
                write!(formatter, "resource sampler worker panicked: {message}")
            }
        }
    }
}

impl std::error::Error for SamplerError {}

/// Background sampler. On Linux it samples `/proc/self`; elsewhere resource
/// fields are reported as unavailable without failing the benchmark.
pub struct ResourceSampler {
    before: Snapshot,
    /// Bytes the start snapshot's own `/proc/self/io` read adds to `rchar`.
    /// The kernel counts a read after it returns, so they land just after the
    /// `rchar` value that read reports, inside the window.
    start_io_read_bytes: u64,
    stop: Option<Sender<()>>,
    handle: Option<JoinHandle<Polled>>,
    measurement_scope: &'static str,
}

/// What the polling worker hands back when it stops.
#[derive(Debug, Clone, Copy, Default)]
struct Polled {
    /// Peak RSS, thread and FD counts seen by the polls.
    peak: Snapshot,
    /// Bytes the polls read, which `rchar` counts exactly (GH-178).
    read_bytes: u64,
}

/// Where a full snapshot reads `/proc/self/io` relative to its other reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IoRead {
    /// Start of a window: the snapshot's other reads land before the counters.
    Last,
    /// End of a window: the snapshot's other reads land after the counters.
    First,
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
        let (before, start_io_read_bytes) = snapshot(IoRead::Last);
        let (stop, thread_stop) = mpsc::channel();
        let handle = thread::spawn(move || {
            // No snapshot on start: `before` already covers the start, and a
            // snapshot here would race the workload, so a test could not tell
            // whether the final snapshot or this one saw a late allocation.
            let mut polled = Polled::default();
            while let Err(mpsc::RecvTimeoutError::Timeout) = thread_stop.recv_timeout(interval) {
                let current = poll(&mut polled.read_bytes);
                polled.peak.rss_kib = option_max(polled.peak.rss_kib, current.rss_kib);
                polled.peak.threads = option_max(polled.peak.threads, current.threads);
                polled.peak.fds = option_max(polled.peak.fds, current.fds);
            }
            polled
        });
        Self {
            before,
            start_io_read_bytes,
            stop: Some(stop),
            handle: Some(handle),
            measurement_scope,
        }
    }

    #[must_use = "finish the sampler and handle measurement failure"]
    pub fn finish(mut self) -> Result<ResourceSummary, SamplerError> {
        let Polled {
            peak,
            read_bytes: poll_read_bytes,
        } = self.stop_and_join()?;
        let (after, _) = snapshot(IoRead::First);
        // Normalise by the CPU count from the same /proc/stat as the totals
        // (GH-178), not `available_parallelism`, which follows the affinity
        // mask or quota while the totals cover every online CPU.
        let cpu_cores = match (
            self.before.process_jiffies,
            after.process_jiffies,
            self.before.total_jiffies,
            after.total_jiffies,
            after.online_cpus.or(self.before.online_cpus),
        ) {
            (
                Some(proc_before),
                Some(proc_after),
                Some(total_before),
                Some(total_after),
                Some(ncpus),
            ) => Some(cpu_cores_for_window(
                proc_after.saturating_sub(proc_before),
                total_after.saturating_sub(total_before),
                ncpus,
            )),
            _ => None,
        };
        let peak_rss_kib = option_max(option_max(peak.rss_kib, self.before.rss_kib), after.rss_kib);
        let peak_threads = option_max(option_max(peak.threads, self.before.threads), after.threads);
        let peak_fds = option_max(option_max(peak.fds, self.before.fds), after.fds);
        let rss_start_mib = self.before.rss_kib.map(kib_to_mib);
        let rss_end_mib = after.rss_kib.map(kib_to_mib);
        let peak_rss_mib = peak_rss_kib.map(kib_to_mib);

        Ok(ResourceSummary {
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
            // rchar counts exactly the bytes read() returns, so the sampler's
            // own reads inside the window are subtracted exactly (GH-178).
            logical_read_bytes: delta(self.before.rchar, after.rchar).map(|bytes| {
                bytes
                    .saturating_sub(poll_read_bytes)
                    .saturating_sub(self.start_io_read_bytes)
            }),
            logical_write_bytes: delta(self.before.wchar, after.wchar),
        })
    }

    fn stop_and_join(&mut self) -> Result<Polled, SamplerError> {
        self.stop.take();
        self.handle
            .take()
            .map(|handle| {
                handle
                    .join()
                    .map_err(|payload| SamplerError::WorkerPanicked(panic_message(payload)))
            })
            .unwrap_or_else(|| Ok(Polled::default()))
    }
}

impl Drop for ResourceSampler {
    fn drop(&mut self) {
        // Cancellation-safe cleanup: error or early-return paths can drop a
        // sampler without calling `finish`. Signal the polling thread to exit
        // and join it so it does not keep running detached past the sampler's
        // lifetime.
        let _ = self.stop_and_join();
    }
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "non-string panic payload".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    #[test]
    fn drop_signals_the_worker_and_waits_for_it_to_exit() {
        // The worker records whether it was told to stop, then lingers on its
        // way out. `exited` can only be true when `drop` returns if drop both
        // signalled the worker (otherwise it times out and records false) and
        // joined it (otherwise drop returns while the worker is still asleep).
        let exited = Arc::new(AtomicBool::new(false));
        let (stop, worker_stop) = mpsc::channel::<()>();
        let worker_exited = Arc::clone(&exited);
        let handle = thread::spawn(move || {
            let signalled = matches!(
                worker_stop.recv_timeout(Duration::from_secs(10)),
                Err(mpsc::RecvTimeoutError::Disconnected)
            );
            thread::sleep(Duration::from_millis(100));
            worker_exited.store(signalled, Ordering::SeqCst);
            Polled::default()
        });
        let sampler = ResourceSampler {
            before: Snapshot::default(),
            start_io_read_bytes: 0,
            stop: Some(stop),
            handle: Some(handle),
            measurement_scope: "in-process",
        };

        drop(sampler);

        assert!(
            exited.load(Ordering::SeqCst),
            "drop returned before the signalled worker exited"
        );
    }

    #[test]
    fn worker_panic_is_reported_instead_of_defaulting_snapshot() {
        let handle = thread::spawn(|| -> Polled { panic!("sampler exploded") });
        let mut sampler = ResourceSampler {
            before: Snapshot::default(),
            start_io_read_bytes: 0,
            stop: None,
            handle: Some(handle),
            measurement_scope: "in-process",
        };

        let error = sampler
            .stop_and_join()
            .expect_err("panic should be reported");
        assert_eq!(
            error,
            SamplerError::WorkerPanicked("sampler exploded".to_owned())
        );
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

/// Read a `/proc` file, adding every byte `read()` returns to `counted`.
///
/// `rchar` counts exactly the bytes `read()` returns, so a reader that counts
/// the same bytes can be subtracted from it exactly, even when a read fails
/// part-way through.
#[cfg(target_os = "linux")]
fn read_counted(path: &str, counted: &mut u64) -> Option<String> {
    use std::io::Read;

    let mut file = std::fs::File::open(path).ok()?;
    let mut contents = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                *counted += read as u64;
                contents.extend_from_slice(&chunk[..read]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    String::from_utf8(contents).ok()
}

/// The cheap per-interval poll: RSS and threads from `/proc/self/status`, and
/// the FD count. Listing `/proc/self/fd` uses `getdents64`, which `rchar` does
/// not count, so only the status read is added to `counted`.
#[cfg(target_os = "linux")]
fn poll(counted: &mut u64) -> Snapshot {
    let (rss_kib, threads) = read_counted("/proc/self/status", counted)
        .and_then(|value| parse_proc_status(&value))
        .map_or((None, None), |(rss, threads)| (Some(rss), Some(threads)));
    Snapshot {
        rss_kib,
        threads,
        fds: fd_count(),
        ..Snapshot::default()
    }
}

#[cfg(target_os = "linux")]
fn fd_count() -> Option<u64> {
    std::fs::read_dir("/proc/self/fd")
        .ok()
        .map(|entries| entries.filter_map(Result::ok).count() as u64)
}

/// A full snapshot for the edge of a window. Returns the snapshot and the
/// bytes its own `/proc/self/io` read adds to `rchar`, which the kernel counts
/// after that read reports its value.
#[cfg(target_os = "linux")]
fn snapshot(io_read: IoRead) -> (Snapshot, u64) {
    let mut io_read_bytes = 0;
    let mut read_io = || {
        read_counted("/proc/self/io", &mut io_read_bytes).and_then(|value| parse_proc_io(&value))
    };
    let io_first = (io_read == IoRead::First).then(&mut read_io).flatten();
    let mut other_reads = 0;
    let (rss_kib, threads) = read_counted("/proc/self/status", &mut other_reads)
        .and_then(|value| parse_proc_status(&value))
        .map_or((None, None), |(rss, threads)| (Some(rss), Some(threads)));
    let process_jiffies =
        read_counted("/proc/self/stat", &mut other_reads).and_then(|value| parse_proc_stat(&value));
    let cpu = read_counted("/proc/stat", &mut other_reads).and_then(|value| parse_cpu_stat(&value));
    let fds = fd_count();
    let io = match io_read {
        IoRead::First => io_first,
        IoRead::Last => read_io(),
    };
    let snapshot = Snapshot {
        rss_kib,
        threads,
        fds,
        process_jiffies,
        total_jiffies: cpu.map(|c| c.total_jiffies),
        online_cpus: cpu.map(|c| c.online_cpus),
        read_bytes: io.map(|c| c.read_bytes),
        write_bytes: io.map(|c| c.write_bytes),
        rchar: io.map(|c| c.rchar),
        wchar: io.map(|c| c.wchar),
    };
    (snapshot, io_read_bytes)
}

#[cfg(not(target_os = "linux"))]
fn poll(_counted: &mut u64) -> Snapshot {
    Snapshot::default()
}

#[cfg(not(target_os = "linux"))]
fn snapshot(_io_read: IoRead) -> (Snapshot, u64) {
    (Snapshot::default(), 0)
}

/// Host CPU accounting parsed from one read of `/proc/stat`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CpuStat {
    /// Sum of the first eight fields of the aggregate `cpu` line (user, nice,
    /// system, idle, iowait, irq, softirq, steal) across every online CPU.
    pub total_jiffies: u64,
    /// Number of `cpuN` lines: the CPUs `total_jiffies` covers.
    pub online_cpus: usize,
}

/// Parse the aggregate jiffies and the online CPU count from `/proc/stat`.
///
/// The CPU count comes from the same document as the totals, so dividing a
/// process's jiffies by `total_jiffies` and scaling by `online_cpus` gives
/// saturated cores even when the process is restricted to fewer CPUs by an
/// affinity mask or quota (where `available_parallelism` would disagree with
/// the host-wide totals).
#[must_use]
pub fn parse_cpu_stat(stat: &str) -> Option<CpuStat> {
    let mut lines = stat.lines();
    let mut fields = lines.next()?.split_whitespace();
    (fields.next()? == "cpu").then_some(())?;
    let total_jiffies = fields
        .take(8)
        .map(str::parse::<u64>)
        .try_fold(0u64, |total, value| Some(total.saturating_add(value.ok()?)))?;
    let online_cpus = lines
        .filter(|line| {
            line.strip_prefix("cpu")
                .and_then(|rest| rest.split_whitespace().next())
                .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()))
        })
        .count();
    (online_cpus > 0).then_some(CpuStat {
        total_jiffies,
        online_cpus,
    })
}
