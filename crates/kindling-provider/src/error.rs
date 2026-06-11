use kindling_store::StoreError;

/// Errors produced by the retrieval provider and orchestrator.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json (de)serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Store(#[from] StoreError),

    #[error("unexpected value {value:?} in column {column}")]
    UnexpectedRowValue { column: &'static str, value: String },
}

pub type ProviderResult<T> = Result<T, ProviderError>;
