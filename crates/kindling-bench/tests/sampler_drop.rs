//! Regression coverage for `ResourceSampler` cancellation safety.
//!
//! Dropping a sampler without calling `finish` must stop and join its
//! background polling thread rather than detaching it (which would leave the
//! thread running until process exit). This lives in its own integration-test
//! binary so the process-wide thread count read from `/proc/self/status` stays
//! deterministic: there are no sibling tests running in parallel threads.

#![cfg(target_os = "linux")]

use kindling_bench::resources::{parse_proc_status, ResourceSampler};
use std::time::Duration;

fn live_thread_count() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let (_, threads) = parse_proc_status(&status)?;
    Some(threads)
}

#[test]
fn dropping_without_finish_stops_the_sampler_thread() {
    let baseline = live_thread_count().expect("read baseline thread count");
    let sampler = ResourceSampler::start(Duration::from_millis(1));

    // Confirm the sampler thread is actually running before we drop it.
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while live_thread_count().is_some_and(|count| count <= baseline)
        && std::time::Instant::now() < deadline
    {
        std::thread::sleep(Duration::from_millis(1));
    }
    let during = live_thread_count().expect("read thread count after sampler start");
    assert!(during > baseline, "sampler thread did not spawn");

    drop(sampler);

    // A successful drop returns only after the sampler thread has been joined.
    let after = live_thread_count().expect("read thread count after sampler drop");
    assert_eq!(after, baseline, "sampler thread was not joined on drop");
}
