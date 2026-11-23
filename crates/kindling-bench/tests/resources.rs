use kindling_bench::resources::{
    cpu_cores_for_window, parse_cpu_stat, parse_proc_io, parse_proc_stat, parse_proc_status,
    CpuStat, ProcIo, ResourceSampler,
};
use std::sync::Mutex;
use std::time::Duration;

/// `/proc/self/io` and RSS are process-wide, and libtest runs tests on
/// parallel threads in one process, so tests that measure a sampler window
/// hold this lock to keep another test's sampler out of their window.
static SAMPLER_WINDOW: Mutex<()> = Mutex::new(());

#[test]
fn idle_window_reports_no_logical_reads_despite_polling() {
    let _window = SAMPLER_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    // About 200 polls at a 1 ms interval. Each poll reads /proc/self/status
    // (about 1.4 KB of rchar), so without exact accounting this window would
    // report hundreds of KiB of phantom reads.
    let sampler = ResourceSampler::start(Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(200));
    let summary = sampler.finish().expect("resource sampler should finish");

    // RSS can be readable while /proc/self/io is not (some sandboxes), so
    // guard on the counter this test measures.
    if summary.logical_read_bytes.is_some() {
        assert_eq!(summary.logical_read_bytes, Some(0));
    }
}

#[test]
fn polled_window_reports_exactly_the_workload_reads() {
    const WORKLOAD_BYTES: usize = 64 * 1024 + 123;
    let _window = SAMPLER_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    let path = std::env::temp_dir().join(format!(
        "kindling-bench-known-reads-{}.bin",
        std::process::id()
    ));
    std::fs::write(&path, vec![7_u8; WORKLOAD_BYTES]).expect("write the workload file");

    // Polls run on both sides of the workload read, so this checks the
    // subtraction removes the sampler's reads and nothing else: an
    // over-subtraction would report fewer bytes than the workload read.
    let sampler = ResourceSampler::start(Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(50));
    let read = std::fs::read(&path).expect("read the workload file");
    std::thread::sleep(Duration::from_millis(50));
    let summary = sampler.finish().expect("resource sampler should finish");
    std::fs::remove_file(&path).ok();

    assert_eq!(read.len(), WORKLOAD_BYTES);
    if summary.logical_read_bytes.is_some() {
        assert_eq!(summary.logical_read_bytes, Some(WORKLOAD_BYTES as u64));
    }
}

#[test]
fn linux_resource_documents_are_parsed() {
    let status = "Name:\tkindling-bench\nVmRSS:\t  12345 kB\nThreads:\t7\n";
    let process_stat = "123 (kindling bench) R 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16";
    let io = "rchar: 100\nwchar: 200\nread_bytes: 4096\nwrite_bytes: 8192\n";

    assert_eq!(parse_proc_status(status), Some((12_345, 7)));
    assert_eq!(parse_proc_stat(process_stat), Some(23));
    assert_eq!(
        parse_proc_io(io),
        Some(ProcIo {
            read_bytes: 4_096,
            write_bytes: 8_192,
            rchar: 100,
            wchar: 200,
        })
    );
}

#[test]
fn cpu_window_is_reported_in_saturated_cores() {
    assert_eq!(cpu_cores_for_window(25, 1_000, 8), 0.2);
    assert_eq!(cpu_cores_for_window(25, 0, 8), 0.0);
}

/// `/proc/stat` for an 8-CPU host where CPUs 0 and 1 are fully busy. Each
/// `cpuN` line sums to `per_cpu` jiffies (user + idle), and the aggregate `cpu`
/// line sums to `8 * per_cpu`.
fn eight_cpu_proc_stat(per_cpu: u64) -> String {
    let mut stat = format!("cpu  {} 0 0 {} 0 0 0 0 0 0\n", per_cpu * 2, per_cpu * 6);
    for cpu in 0..8 {
        let busy = if cpu < 2 { per_cpu } else { 0 };
        stat.push_str(&format!(
            "cpu{cpu} {busy} 0 0 {} 0 0 0 0 0 0\n",
            per_cpu - busy
        ));
    }
    stat.push_str("intr 0\nctxt 0\nbtime 0\nprocesses 1\n");
    stat
}

#[test]
fn proc_stat_reports_the_cpu_count_its_totals_cover() {
    let parsed = parse_cpu_stat(&eight_cpu_proc_stat(100)).expect("fixture should parse");

    assert_eq!(
        parsed,
        CpuStat {
            total_jiffies: 800,
            online_cpus: 8
        }
    );
}

#[test]
fn saturating_two_of_eight_cpus_reports_two_cores() {
    // A process pinned to 2 of 8 CPUs saturates both for the whole window. The
    // host-wide totals advance by 8 * 1000 jiffies and the process by 2 * 1000.
    // The core count must come from the same /proc/stat as the totals, not from
    // `available_parallelism`, which is 2 under that affinity mask and would
    // report 0.5 cores.
    let before = parse_cpu_stat(&eight_cpu_proc_stat(1_000)).expect("before parses");
    let after = parse_cpu_stat(&eight_cpu_proc_stat(2_000)).expect("after parses");

    let cores = cpu_cores_for_window(
        2_000,
        after.total_jiffies - before.total_jiffies,
        after.online_cpus,
    );

    assert_eq!(cores, 2.0);
}

#[test]
fn proc_stat_without_per_cpu_lines_is_rejected() {
    assert_eq!(parse_cpu_stat("cpu  1 2 3 4 5 6 7 8 0 0\nintr 0\n"), None);
    assert_eq!(parse_cpu_stat("intr 0\n"), None);
}

#[test]
fn sampled_peak_includes_the_final_snapshot() {
    const BUFFER_MIB: usize = 16;
    let _window = SAMPLER_WINDOW.lock().unwrap_or_else(|e| e.into_inner());
    // An hour-long interval means no poll runs inside this window, and the
    // worker takes no snapshot of its own on start. So the only snapshot that
    // can see the buffer is the one `finish` takes; #270's interruptible stop
    // keeps `finish` from waiting out the interval.
    let sampler = ResourceSampler::start(Duration::from_secs(3_600));
    // Write every byte: untouched zero pages from calloc are not resident and
    // would not show up in RSS at all.
    let buffer = vec![0xA5_u8; BUFFER_MIB * 1024 * 1024];
    std::hint::black_box(&buffer);
    let summary = sampler.finish().expect("resource sampler should finish");
    drop(buffer);

    if summary.available {
        let over_start = summary
            .peak_rss_over_start_mib
            .expect("peak over start is reported when RSS is available");
        assert!(
            over_start >= BUFFER_MIB as f64 * 0.9,
            "peak RSS over start was {over_start:.2} MiB; the final snapshot should see the \
             {BUFFER_MIB} MiB buffer"
        );
        assert!(summary.peak_rss_mib >= summary.rss_start_mib);
        assert!(summary.peak_rss_mib >= summary.rss_end_mib);
        assert_eq!(
            summary.peak_rss_over_start_mib,
            summary
                .peak_rss_mib
                .zip(summary.rss_start_mib)
                .map(|(peak, start)| (peak - start).max(0.0))
        );
        assert_eq!(
            summary.peak_threads_over_start,
            summary
                .peak_threads
                .zip(summary.threads_start)
                .map(|(peak, start)| peak.saturating_sub(start))
        );
        assert_eq!(
            summary.peak_file_descriptors_over_start,
            summary
                .peak_file_descriptors
                .zip(summary.file_descriptors_start)
                .map(|(peak, start)| peak.saturating_sub(start))
        );
    }
}
