use testcontainers::clients;
use crate::dto::{Service, ServiceType};
use crate::repo;
use crate::repo::test::start_postgres;

const TEST_NAME: &str = "SadBot";

#[tokio::test]
async fn test_services() -> anyhow::Result<()> {
    let docker = clients::Cli::default();
    let (_container, db) = start_postgres(&docker).await;
    let services = repo::Services::new(db);

    let service = Service {
        name: TEST_NAME.to_owned(),
        service_type: ServiceType::TelegramBot,
    };

    assert!(services.get_id(&service).await?.is_none());

    let created_id = services.create(ServiceType::TelegramBot, TEST_NAME).await?;
    let fetched_id = services.get_id(&service).await?.expect("fetched_id must be");
    let fetched_cached_id = services.get_id(&service).await?.expect("fetched_cached_id must be");

    assert_eq!(created_id, fetched_id);
    assert_eq!(created_id, fetched_cached_id);

    Ok(())
}
