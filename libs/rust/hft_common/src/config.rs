//! Env-var helpers for service configuration.
//!
//! # Pattern used in every service
//! ```rust,ignore
//! use hft_common::config::{load_dotenv, require_env, env_or, env_parse};
//!
//! pub fn from_env() -> hft_common::Result<Config> {
//!     load_dotenv();
//!     Ok(Config {
//!         kafka_brokers: env_or("KAFKA_BROKERS", "redpanda:9092"),
//!         database_url:  require_env("DATABASE_URL")?,
//!         max_notional:  env_parse("LIMIT_NOTIONAL_PER_SYMBOL", 5000.0_f64)?,
//!     })
//! }
//! ```

use std::str::FromStr;

use crate::error::{AppError, Result};

/// Load a `.env` file from the current directory if one exists.
///
/// Silently ignores a missing file; does **not** override existing env vars.
/// Call once at the start of `from_env()`.
pub fn load_dotenv() {
    dotenvy::dotenv().ok();
}

/// Read a **required** env var.
///
/// Returns [`AppError::Config`] with a descriptive message if the var is unset.
pub fn require_env(key: &str) -> Result<String> {
    std::env::var(key).map_err(|_| AppError::config(format!("required env var `{key}` is not set")))
}

/// Read an env var, falling back to `default` if it is unset or empty.
pub fn env_or(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_owned())
}

/// Read and parse an optional env var, returning `default` if unset.
///
/// Returns [`AppError::Config`] if the var is set but cannot be parsed.
pub fn env_parse<T>(key: &str, default: T) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Err(_) => Ok(default),
        Ok(raw) => raw
            .parse::<T>()
            .map_err(|e| AppError::config(format!("`{key}` = {raw:?} is invalid: {e}"))),
    }
}

/// Read and parse a **required** env var.
///
/// Returns [`AppError::Config`] if the var is unset or cannot be parsed.
pub fn require_env_parse<T>(key: &str) -> Result<T>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let raw = require_env(key)?;
    raw.parse::<T>()
        .map_err(|e| AppError::config(format!("`{key}` = {raw:?} is invalid: {e}")))
}

/// Read a secret from a file path specified by an env var.
///
/// Used for Binance API keys mounted as Docker secrets:
/// `BINANCE_API_KEY_FILE=/run/secrets/binance_key`.
pub fn read_secret_file(env_key: &str) -> Result<String> {
    let path = require_env(env_key)?;
    std::fs::read_to_string(&path)
        .map(|s| s.trim().to_owned())
        .map_err(|e| AppError::config(format!("cannot read secret file `{path}`: {e}")))
}
