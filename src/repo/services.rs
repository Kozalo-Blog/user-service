use std::collections::HashMap;
use std::sync::Arc;
use axum::async_trait;
use tokio::sync::RwLock;
use crate::dto::{Service, ServiceType};

#[async_trait]
pub trait Services {
    async fn create(&self, service_type: ServiceType, name: &str) -> Result<i32, sqlx::Error>;
    async fn get_id(&self, service: &Service) -> Result<Option<i32>, sqlx::Error>;
}

pub struct ServicesPostgres {
    pool: sqlx::Pool<sqlx::Postgres>,
    id_cache: Arc<RwLock<HashMap<ServiceKey, i32>>>
}

#[derive(Hash, Eq, PartialEq)]
struct ServiceKey(String, ServiceType);

impl From<Service> for ServiceKey {
    fn from(value: Service) -> Self {
        Self(value.name, value.service_type)
    }
}

impl ServicesPostgres {
    pub fn new(pool: sqlx::Pool<sqlx::Postgres>) -> Self {
        Self {
            pool,
            id_cache: Arc::new(RwLock::new(HashMap::new()))
        }
    }
}

#[async_trait]
impl Services for ServicesPostgres {
    async fn create(&self, service_type: ServiceType, name: &str) -> Result<i32, sqlx::Error> {
        log::info!("creation of a service '{name}' of type {service_type:?}...");
        sqlx::query_scalar!("INSERT INTO Services (type, name) VALUES ($1, $2) RETURNING id",
                service_type as ServiceType, name)
            .fetch_one(&self.pool)
            .await
    }

    async fn get_id(&self, service: &Service) -> Result<Option<i32>, sqlx::Error> {
        let cached_id = {
            self.id_cache
                .read().await
                .get(&service.clone().into())
                .map(|v| *v)
        };
        let id = if cached_id.is_none() {
            log::info!("fetching a service id for '{}' [type={:?}]", service.name, service.service_type);
            let fetched_id = sqlx::query_scalar!("SELECT id FROM Services WHERE name = $1 AND type = $2",
                    &service.name, service.service_type as ServiceType)
                .fetch_optional(&self.pool)
                .await?;
            if let Some(id) = fetched_id {
                self.id_cache
                    .write().await
                    .insert(service.clone().into(), id);
            }
            fetched_id
        } else {
            log::debug!("cached id {cached_id:?} is used for '{}' [type={:?}]", service.name, service.service_type);
            cached_id
        };
        Ok(id)
    }
}
