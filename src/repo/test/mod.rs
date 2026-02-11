use std::str::FromStr;
use sqlx::{Pool, Postgres};
use testcontainers::{runners::AsyncRunner, ContainerAsync, GenericImage, ImageExt};
use testcontainers::core::WaitFor;
use url::Url;
use crate::repo;

mod services;
mod users;
mod export;

pub use export::*;

const POSTGRES_USER: &str = "test";
const POSTGRES_PASSWORD: &str = "test_pw";
const POSTGRES_DB: &str = "test_db";
const POSTGRES_PORT: u16 = 5432;

pub async fn start_postgres() -> (ContainerAsync<GenericImage>, Pool<Postgres>) {
    let postgres_image = GenericImage::new("postgres", "latest")
        .with_env_var("POSTGRES_USER", POSTGRES_USER)
        .with_env_var("POSTGRES_PASSWORD", POSTGRES_PASSWORD)
        .with_env_var("POSTGRES_DB", POSTGRES_DB)
        .with_ready_conditions(vec![
            WaitFor::message_on_stdout("PostgreSQL init process complete; ready for start up.")
        ]);

    let postgres_container = postgres_image.start().await.expect("failed to start postgres container");
    let postgres_port = postgres_container.get_host_port_ipv4(POSTGRES_PORT).await.expect("failed to get postgres port");
    let db_url = Url::from_str(&format!("postgres://{POSTGRES_USER}:{POSTGRES_PASSWORD}@localhost:{postgres_port}/{POSTGRES_DB}"))
        .expect("invalid database URL");
    let conf = repo::DatabaseConfig{
        url: db_url,
        max_connections: 5,
    };
    let pool = repo::establish_database_connection(&conf)
        .await.expect("couldn't establish a database connection");
    (postgres_container, pool)
}
