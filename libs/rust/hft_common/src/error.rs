//! Domain error type and Result alias used across all HFT services.
//!
//! Service-level code that needs richer context should still use `anyhow`
//! (via `anyhow::Context`) inside `main` and convert at boundaries.

use thiserror::Error;

/// Top-level error enum for HFT services.
///
/// Add a new variant per distinct failure *category*, not per call-site.
/// Use [`AppError::Other`] for one-off errors that don't need pattern-matching.
#[derive(Debug, Error)]
pub enum AppError {
    /// Environment / config parsing failure.
    #[error("config error: {0}")]
    Config(String),

    /// Kafka / Redpanda producer or consumer error.
    #[error("kafka error: {0}")]
    Kafka(String),

    /// Redis error.
    #[error("redis error: {0}")]
    Redis(String),

    /// Protobuf encode / decode failure.
    #[error("codec error: {0}")]
    Codec(String),

    /// Database (Postgres / ClickHouse) error.
    #[error("database error: {0}")]
    Database(String),

    /// Standard I/O error (file reads, secrets, sockets).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Catch-all for errors that don't fit a category above.
    #[error("{0}")]
    Other(String),
}

impl AppError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }
    pub fn kafka(msg: impl Into<String>) -> Self {
        Self::Kafka(msg.into())
    }
    pub fn redis(msg: impl Into<String>) -> Self {
        Self::Redis(msg.into())
    }
    pub fn codec(msg: impl Into<String>) -> Self {
        Self::Codec(msg.into())
    }
    pub fn database(msg: impl Into<String>) -> Self {
        Self::Database(msg.into())
    }
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Convenience alias: `hft_common::Result<T>` = `Result<T, AppError>`.
pub type Result<T> = std::result::Result<T, AppError>;
