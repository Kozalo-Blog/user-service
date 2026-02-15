pub mod users;
pub mod services;
pub mod error;

#[cfg(test)]
pub mod test;

use url::Url;
use crate::env::{get_mandatory_value, get_value_or_default};
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

#[cfg_attr(test, derive(derive_more::Constructor))]
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
