use chrono::{TimeZone, Utc};
use derive_more::From;
use crate::dto::{SavedUser, Code, ExternalUser, error::TypeConversionError};
use crate::repository;
use crate::repo::error::RepoError;

#[derive(sqlx::FromRow)]
struct UserInternal {
    id: i64,
    name: Option<String>,
    language_code: Option<String>,
    location: Option<Vec<f64>>,
    premium_till: Option<chrono::DateTime<Utc>>
}

impl TryFrom<UserInternal> for SavedUser {
    type Error = TypeConversionError;

    fn try_from(value: UserInternal) -> Result<Self, Self::Error> {
        let language_code = value.language_code
            .map(|code| code.try_into())
            .transpose()
            .map_err(|e| TypeConversionError::new(e))?;

        let location = value.location
            .map(|loc| loc.try_into())
            .transpose()
            .map_err(|e| TypeConversionError::new(e))?;

        Ok(Self {
            id: value.id,
            name: value.name,
            language_code,
            location,
            premium_till: value.premium_till,
        })
    }
}

#[derive(Copy, Clone)]
pub enum UserId {
    Internal(i64),
    External(i64),
}

#[derive(From)]
pub enum UpdateTarget {
    Language(Code),
    Location { latitude: f64, longitude: f64 },
    Premium { till: chrono::DateTime<Utc> },
}

repository!(Users,
    pub async fn get(&self, id: UserId) -> Result<Option<SavedUser>, RepoError<TypeConversionError>> {
        let result = match id {
            UserId::Internal(id) => sqlx::query_as!(UserInternal,
                    "SELECT id, name, language_code, location, premium_till FROM Users WHERE id = $1", id)
                .fetch_optional(&self.pool)
                .await,
            UserId::External(external_id) => sqlx::query_as!(UserInternal,
                    "SELECT id, name, language_code, location, premium_till FROM Users u
                    JOIN User_Service_Mappings usm ON u.id = usm.user_id
                    WHERE external_id = $1", external_id)
                .fetch_optional(&self.pool)
                .await,
        };
        match result {
            Ok(Some(user)) => user.try_into().map_err(|e| RepoError::Other(e)).map(Some),
            Ok(None) => Ok(None),
            Err(e) => Err(RepoError::Database(e.into()))
        }
    }
,
    pub async fn register(&self, user: ExternalUser, service_id: i32) -> Result<i64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let user_id = sqlx::query_scalar!("INSERT INTO Users (name) VALUES ($1) RETURNING id",
                user.name)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query_scalar!("INSERT INTO User_Service_Mappings (user_id, service_id, external_id) VALUES ($1, $2, $3)",
                user_id, service_id, user.external_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(user_id)
    }
,
    pub async fn get_user_id(&self, service_id: i32, external_id: i64) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar!("SELECT user_id FROM User_Service_Mappings
                WHERE service_id = $1 AND external_id = $2",
                service_id, external_id)
            .fetch_optional(&self.pool)
            .await
    }
,
    pub async fn update_value(&self, user_id: i64, value: UpdateTarget) -> Result<(), sqlx::Error> {
        match value {
            UpdateTarget::Language(code) => self.update_language(user_id, code).await,
            UpdateTarget::Location { latitude, longitude } => self.update_location(user_id, latitude, longitude).await,
            UpdateTarget::Premium { till } => self.update_premium(user_id, till).await,
        }
    }
,
    async fn update_language(&self, user_id: i64, language: Code) -> Result<(), sqlx::Error> {
        let lang_code: String = language.into();
        sqlx::query!("UPDATE Users SET language_code = $2 WHERE id = $1", user_id, lang_code)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
,
    async fn update_location(&self, user_id: i64, latitude: f64, longitude: f64) -> Result<(), sqlx::Error> {
        sqlx::query!("UPDATE Users SET location = ARRAY[$2::float8, $3::float8] WHERE id = $1", user_id, latitude, longitude)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
,
    async fn update_premium<T>(&self, user_id: i64, till: chrono::DateTime<T>) -> Result<(), sqlx::Error>
    where
        T: TimeZone + Send + Sync,
        <T as TimeZone>::Offset: Send + Sync
    {
        sqlx::query!("UPDATE Users SET premium_till = $2 WHERE id = $1", user_id, till)
            .execute(&self.pool)
            .await
            .map(|_| ())
    }
);
