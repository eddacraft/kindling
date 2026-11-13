//! Regression coverage for `ResourceSampler` cancellation safety.
//!
//! Dropping a sampler without calling `finish` must stop and join its
//! background polling thread rather than detaching it (which would leave the
//! thread running until process exit). This lives in its own integration-test
//! binary so the process-wide thread count read from `/proc/self/status` stays
//! deterministic: there are no sibling tests running in parallel threads.

#[cfg(target_os = "linux")]
use kindling_bench::resources::{parse_proc_status, ResourceSampler};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
fn live_thread_count() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| parse_proc_status(&status))
        .map_or(0, |(_, threads)| threads)
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_without_finish_stops_the_sampler_thread() {
    let baseline = live_thread_count();
    let sampler = ResourceSampler::start(Duration::from_millis(1));

    // Confirm the sampler thread is actually running before we drop it.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while live_thread_count() <= baseline && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(
        live_thread_count() > baseline,
        "sampler thread did not spawn"
    );

    drop(sampler);

    // Dropping joins the sampler thread. Poll (bounded) for the thread count to
    // return to baseline: a joined thread's `/proc` entry can linger briefly
    // during teardown, but a genuinely leaked thread never disappears.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while live_thread_count() > baseline && std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        live_thread_count(),
        baseline,
        "sampler thread was not joined on drop"
    );
}
