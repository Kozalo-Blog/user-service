use std::str::FromStr;
use sqlx::{Pool, Postgres};
use testcontainers::{clients, Container, GenericImage};
use testcontainers::core::WaitFor;
use url::Url;
use crate::repo;

mod services;
mod users;

const POSTGRES_USER: &str = "test";
const POSTGRES_PASSWORD: &str = "test_pw";
const POSTGRES_DB: &str = "test_db";
const POSTGRES_PORT: u16 = 5432;

pub async fn start_postgres(docker: &clients::Cli) -> (Container<GenericImage>, Pool<Postgres>) {
    let postgres_image = GenericImage::new("postgres", "latest")
        .with_exposed_port(POSTGRES_PORT)
        .with_wait_for(WaitFor::message_on_stdout("PostgreSQL init process complete; ready for start up."))
        .with_wait_for(WaitFor::message_on_stdout("PostgreSQL init process complete; ready for start up."))
        .with_env_var("POSTGRES_USER", POSTGRES_USER)
        .with_env_var("POSTGRES_PASSWORD", POSTGRES_PASSWORD)
        .with_env_var("POSTGRES_DB", POSTGRES_DB);

    let postgres_container = docker.run(postgres_image);
    let postgres_port = postgres_container.get_host_port_ipv4(POSTGRES_PORT);
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
