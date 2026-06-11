use kindling_types::Id;

/// Errors produced by the SQLite store.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json (de)serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("capsule {0} not found or already closed")]
    CapsuleNotOpen(Id),

    #[error("summary {summary_id} not found for capsule {capsule_id}")]
    SummaryNotFound { summary_id: Id, capsule_id: Id },

    #[error("pin {0} not found")]
    PinNotFound(Id),

    #[error("observation {0} not found")]
    ObservationNotFound(Id),

    #[error(
        "database schema version {found} is below the minimum compatible version \
         {min_compatible} — run the TypeScript migration runner to upgrade it"
    )]
    SchemaTooOld { found: i64, min_compatible: i64 },

    #[error(
        "database schema version {found} is newer than the supported version {supported} — \
         upgrade this kindling binary"
    )]
    SchemaTooNew { found: i64, supported: i64 },

    #[error("database has no schema and the connection is read-only")]
    UninitializedDatabase,

    #[error("unexpected value {value:?} in column {column}")]
    UnexpectedRowValue { column: &'static str, value: String },
}

pub type StoreResult<T> = Result<T, StoreError>;
