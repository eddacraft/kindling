//! Workload sizes for quick checks, representative local runs, and stress runs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub name: String,
    pub cold_starts: usize,
    pub warmup_operations: usize,
    pub write_operations: usize,
    pub seed_observations: usize,
    pub read_repetitions: usize,
    pub page_limit: u32,
    pub concurrent_writers: usize,
    pub writes_per_task: usize,
    pub spool_operations: usize,
}

impl Profile {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "smoke" => Some(Self {
                name: "smoke".to_string(),
                cold_starts: 3,
                warmup_operations: 20,
                write_operations: 200,
                seed_observations: 2_000,
                read_repetitions: 3,
                page_limit: 100,
                concurrent_writers: 4,
                writes_per_task: 50,
                spool_operations: 100,
            }),
            "standard" => Some(Self {
                name: "standard".to_string(),
                cold_starts: 10,
                warmup_operations: 100,
                write_operations: 2_000,
                seed_observations: 20_000,
                read_repetitions: 5,
                page_limit: 500,
                concurrent_writers: 8,
                writes_per_task: 500,
                spool_operations: 1_000,
            }),
            "stress" => Some(Self {
                name: "stress".to_string(),
                cold_starts: 25,
                warmup_operations: 500,
                write_operations: 20_000,
                seed_observations: 200_000,
                read_repetitions: 10,
                page_limit: 1_000,
                concurrent_writers: 16,
                writes_per_task: 2_000,
                spool_operations: 100_000,
            }),
            _ => None,
        }
    }
}
