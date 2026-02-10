use std::collections::HashMap;
use std::sync::Arc;
use async_trait::async_trait;
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
    #[tracing::instrument(skip(self), fields(service_type = ?service_type, name = %name))]
    async fn create(&self, service_type: ServiceType, name: &str) -> Result<i32, sqlx::Error> {
        tracing::info!("Creating new service");
        let result = sqlx::query_scalar!("INSERT INTO Services (type, name) VALUES ($1, $2) RETURNING id",
                service_type as ServiceType, name)
            .fetch_one(&self.pool)
            .await?;
        tracing::info!(service_id = %result, "Service created successfully");
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(service_name = %service.name, service_type = ?service.service_type))]
    async fn get_id(&self, service: &Service) -> Result<Option<i32>, sqlx::Error> {
        let cached_id = {
            self.id_cache
                .read().await
                .get(&service.clone().into())
                .map(|v| *v)
        };
        let id = if cached_id.is_none() {
            tracing::debug!("Cache miss - fetching service ID from database");
            let fetched_id = sqlx::query_scalar!("SELECT id FROM Services WHERE name = $1 AND type = $2",
                    &service.name, service.service_type as ServiceType)
                .fetch_optional(&self.pool)
                .await?;
            if let Some(id) = fetched_id {
                tracing::debug!(service_id = %id, "Service ID found, caching");
                self.id_cache
                    .write().await
                    .insert(service.clone().into(), id);
            } else {
                tracing::debug!("Service not found in database");
            }
            fetched_id
        } else {
            tracing::debug!(cached_id = ?cached_id, "Cache hit - using cached service ID");
            cached_id
        };
        Ok(id)
    }
}
