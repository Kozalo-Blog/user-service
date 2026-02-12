pub mod users;
pub mod services;
pub mod error;

#[cfg(test)]
pub mod test;

use std::str::FromStr;
use anyhow::anyhow;
use derive_more::Constructor;
use url::Url;
use crate::repo::services::{Services, ServicesPostgres};
use crate::repo::users::{Users, UsersPostgres};

#[derive(Clone)]
pub struct DatabaseConfig {
    pub url: Url,
    pub max_connections: u32
}

impl DatabaseConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            url: get_mandatory_value("DATABASE_URL")?,
            max_connections: get_value_or_default("DATABASE_MAX_CONNECTIONS", 10)
        })
    }
}

#[cfg_attr(test, derive(Constructor))]
pub struct Repositories<U, S>
where
    U: Users,
    S: Services,
{
    pub users: U,
    pub services: S,
}

pub type ProdRepositories = Repositories<UsersPostgres, ServicesPostgres>;

impl ProdRepositories {
    pub fn from_db(db: sqlx::Pool<sqlx::Postgres>) -> Self {
        Self {
            users: UsersPostgres::new(db.clone()),
            services: ServicesPostgres::new(db),
        }
    }
}

pub async fn establish_database_connection(config: &DatabaseConfig) -> Result<sqlx::Pool<sqlx::Postgres>, anyhow::Error> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(config.max_connections)
        .connect(config.url.as_str()).await?;
    sqlx::migrate!().run(&pool).await?;
    Ok(pool)
}

fn get_mandatory_value<T, E>(key: &str) -> anyhow::Result<T>
    where
        T: FromStr<Err = E>,
        E: std::error::Error + Send + Sync + 'static
{
    std::env::var(key)?
        .parse()
        .map_err(|e: E| anyhow!(e))
}

fn get_value_or_default<T, E>(key: &str, default: T) -> T
    where
        T: FromStr<Err = E> + std::fmt::Display,
        E: std::error::Error + Send + Sync + 'static
{
    std::env::var(key)
        .map_err(|e| {
            log::warn!("no value was found for an optional environment variable {key}, using the default value {default}");
            anyhow!(e)
        })
        .and_then(|v| v.parse()
            .map_err(|e: E| {
                log::warn!("invalid value of the {key} environment variable, using the default value {default}");
                anyhow!(e)
            }))
        .unwrap_or(default)
}
