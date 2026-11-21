use kindling_bench::resources::{
    cpu_cores_for_window, parse_proc_io, parse_proc_stat, parse_proc_status, ProcIo,
    ResourceSampler,
};
use std::time::Duration;

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

#[test]
fn sampled_peak_includes_the_final_snapshot() {
    let sampler = ResourceSampler::start(Duration::from_millis(1));
    let allocation = vec![0_u8; 2 * 1024 * 1024];
    std::hint::black_box(&allocation);
    let summary = sampler.finish().expect("resource sampler should finish");

    if summary.available {
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
