pub mod moneymax;
pub mod state_store;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DbError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Business rule violation: {0}")]
    BusinessRule(String),
    #[error("Not found: {0}")]
    NotFound(String),
}
