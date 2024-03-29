use std::collections::HashMap;
use std::sync::Arc;
use axum::async_trait;
use chrono::{DateTime, Utc};
use num_traits::PrimInt;
use tokio::sync::Mutex;
use crate::dto::{ExternalUser, PremiumVariant, SavedUser, Service, ServiceType};
use crate::dto::error::TypeConversionError;
use crate::repo::error::RepoError;
use crate::repo::Repositories;
use crate::repo::services::Services;
use crate::repo::users::{UpdateTarget, UserId, Users};

pub trait CtorWithData<K: PrimInt, V> {
    fn with_data(data: HashMap<K, V>) -> Self;
}

macro_rules! create_mock_struct {
    ($type_name:ident, $id_type:ty, $key_type:ty, $value_type:ty, $data_field:ident) => {
        pub struct $type_name {
            id_seq: Arc<Mutex<dyn Iterator<Item=$id_type> + Send + Sync + 'static>>,
            $data_field: Arc<Mutex<HashMap<$key_type, $value_type>>>
        }

        impl Default for $type_name {
            fn default() -> Self {
                Self {
                    id_seq: Arc::new(Mutex::new((1..).into_iter())),
                    $data_field: Arc::new(Mutex::new(HashMap::new())),
                }
            }
        }

        impl CtorWithData<$key_type, $value_type> for $type_name {
            fn with_data(data: HashMap<$key_type, $value_type>) -> Self {
                Self {
                    $data_field: Arc::new(Mutex::new(data)),
                    ..Self::default()
                }
            }
        }

        impl $type_name {
            async fn gen_id(&self) -> $id_type {
                self.id_seq.lock().await
                    .next().expect("The range is endless")
            }
        }
    };
}

create_mock_struct!(ServicesMock, i32, i32, Service, services);
create_mock_struct!(UsersMock, i64, ExternalId, SavedUser, users);

#[async_trait]
impl Services for ServicesMock {
    async fn create(&self, service_type: ServiceType, name: &str) -> Result<i32, sqlx::Error> {
        log::info!("ServiceMock:create: {name} ({service_type:?})");
        let id = self.gen_id().await;
        let service = (name.to_string(), service_type).into();
        self.services.lock().await
            .insert(id, service);
        Ok(id)
    }

    async fn get_id(&self, service: &Service) -> Result<Option<i32>, sqlx::Error> {
        // since this is just a mock, use the simplest O(n) search
        self.services.lock().await
            .iter()
            .filter(|(_, srv)| *srv == service)
            .take(1)
            .map(|(id, _)| Ok(*id))
            .next()
            .transpose()
    }
}

pub type ExternalId = i64;

#[async_trait]
impl Users for UsersMock {
    async fn get(&self, id: UserId) -> Result<Option<SavedUser>, RepoError<TypeConversionError>> {
        let users = self.users.lock().await;
        match id {
            UserId::Internal(internal_id) => users.iter()
                .map(|(_, usr)| usr.clone())
                .filter(|usr| usr.id == internal_id)
                .map(Ok)
                .take(1)
                .next()
                .transpose(),
            UserId::External(external_id) => users.get(&external_id)
                .map(|usr_ref| Ok(usr_ref.clone()))
                .transpose()
        }
    }

    async fn register(&self, user: ExternalUser, service_id: i32, _: serde_json::Value) -> Result<i64, sqlx::Error> {
        log::info!("UsersMock:register: {user:?} (service_id = {service_id})");
        let id = self.gen_id().await;
        let saved_user = SavedUser {
            id,
            name: user.name,
            language_code: None,
            location: None,
            premium_till: None,
        };

        self.users.lock().await
            .insert(user.external_id as ExternalId, saved_user)
            .map(|_| Ok(id))
            .unwrap_or(Ok(id))
    }

    async fn get_user_id(&self, _: i32, external_id: i64) -> Result<Option<i64>, sqlx::Error> {
        let user_id = external_id as ExternalId;
        self.users.lock().await
            .get(&user_id)
            .map(|usr| Ok(usr.id))
            .transpose()
    }

    async fn update_value(&self, user_id: i64, target: UpdateTarget) -> Result<(), sqlx::Error> {
        log::info!("UsersMock:update_value for {user_id} - {target:?}");
        self.modify_user(user_id, |user| {
            match target {
                UpdateTarget::Language(code) => { user.language_code.replace(code); },
                UpdateTarget::Location { latitude, longitude } => { user.location.replace((latitude, longitude).into()); },
            }
        }).await
    }

    async fn activate_premium(&self, user_id: i64, variant: PremiumVariant) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
        log::info!("UsersMock:activate_premium for {user_id} for {}", variant as u32);
        let premium_active = self.find_user(user_id).await?
            .premium_till.is_some();
        if premium_active {
            return Ok(None)
        }

        self.modify_user(user_id, |user| {
            user.premium_till.replace(variant.into());
        }).await?;
        Ok(Some(variant.into()))
    }
}

impl UsersMock {
    async fn find_user(&self, id: i64) -> Result<SavedUser, sqlx::Error> {
        let external_id = self.find_external_id(id).await?;
        let user = self.users.lock().await
            .get(&external_id)
            .expect("user must be in the HashMap here!")
            .clone();
        Ok(user)
    }

    async fn modify_user(&self, id: i64, mut action: impl FnMut(&mut SavedUser)) -> Result<(), sqlx::Error> {
        let external_id = self.find_external_id(id).await?;
        let mut users = self.users.lock().await;
        let user= users.get_mut(&external_id)
            .expect("user must be in the HashMap here!");
        Ok(action(user))
    }

    async fn find_external_id(&self, id: i64) -> Result<ExternalId, sqlx::Error> {
        self.users.lock().await.iter()
            .filter(|(_, u)| u.id == id)
            .map(|(ext_id, _)| *ext_id)
            .take(1)
            .next()
            .ok_or(sqlx::Error::RowNotFound)
    }
}

pub fn mock_repositories() -> Repositories {
    Repositories {
        services: Box::new(ServicesMock::default()),
        users: Box::new(UsersMock::default()),
    }
}
