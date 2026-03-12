//! Environment variable utilities

use std::str::FromStr;
use anyhow::anyhow;

/// Get a mandatory environment variable value
///
/// # Errors
/// Returns error if:
/// - Variable is not set
/// - Value cannot be parsed to type T
///
/// # Example
/// ```ignore
/// let database_url: Url = get_mandatory_value("DATABASE_URL")?;
/// ```
pub fn get_mandatory_value<T, E>(key: &str) -> anyhow::Result<T>
where
    T: FromStr<Err = E>,
    E: std::error::Error + Send + Sync + 'static,
{
    std::env::var(key)?
        .parse()
        .map_err(|e: E| anyhow!(e))
}

/// Get an optional environment variable value with default fallback
///
/// Logs warnings when:
/// - Variable is not set
/// - Value cannot be parsed
///
/// # Example
/// ```ignore
/// let max_connections = get_value_or_default("DATABASE_MAX_CONNECTIONS", 10);
/// let endpoint = get_value_or_default("OTEL_ENDPOINT", "http://localhost:4317".to_string());
/// ```
pub fn get_value_or_default<T, E>(key: &str, default: T) -> T
where
    T: FromStr<Err = E> + std::fmt::Display,
    E: std::error::Error + Send + Sync + 'static,
{
    std::env::var(key)
        .map_err(|e| {
            tracing::warn!(
                key = %key,
                default = %default,
                "Environment variable not set, using default"
            );
            anyhow!(e)
        })
        .and_then(|v| {
            v.parse().map_err(|e: E| {
                tracing::warn!(
                    key = %key,
                    value = %v,
                    default = %default,
                    error = %e,
                    "Invalid environment variable value, using default"
                );
                anyhow!(e)
            })
        })
        .unwrap_or(default)
}
