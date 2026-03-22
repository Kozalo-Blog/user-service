use chrono::{DateTime, Utc};
use derive_more::{Constructor, From};
use num_traits::Zero;
use sqlx::postgres::PgQueryResult;
use crate::dto::{SavedUser, Code, ExternalUser, error::TypeConversionError, Location, PremiumVariant};
use crate::repo::error::RepoError;

#[derive(sqlx::FromRow)]
struct UserInternal {
    id: i64,
    name: Option<String>,
    language_code: Option<String>,
    location: Option<Vec<f64>>,
    premium_till: Option<DateTime<Utc>>
}

impl TryFrom<UserInternal> for SavedUser {
    type Error = TypeConversionError;

    fn try_from(value: UserInternal) -> Result<Self, Self::Error> {
        let language_code = value.language_code
            .map(|code| code.try_into())
            .transpose()
            .map_err(TypeConversionError::new)?;

        let location = value.location
            .map(|loc| loc.try_into())
            .transpose()
            .map_err(TypeConversionError::new)?;

        Ok(Self {
            id: value.id,
            name: value.name,
            language_code,
            location,
            premium_till: value.premium_till,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub enum UserId {
    Internal(i64),
    External(i64),
}

impl std::fmt::Display for UserId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UserId::Internal(id) => write!(f, "internal:{id}"),
            UserId::External(id) => write!(f, "external:{id}"),
        }
    }
}

#[derive(Debug, From)]
pub enum UpdateTarget {
    Language(Code),
    Location { latitude: f64, longitude: f64 },
}

impl From<Location> for UpdateTarget {
    fn from(value: Location) -> Self {
        let Location { latitude, longitude } = value;
        Self::Location { latitude, longitude }
    }
}

pub trait Users: Send + Sync {
    fn get(&self, id: UserId) -> impl Future<Output = Result<Option<SavedUser>, RepoError<TypeConversionError>>> + Send;
    fn register(&self, user: ExternalUser, service_id: i32, consent_info: serde_json::Value) -> impl Future<Output = Result<i64, RepoError<TypeConversionError>>> + Send;
    fn get_user_id(&self, service_id: i32, external_id: i64) -> impl Future<Output = Result<Option<i64>, RepoError<TypeConversionError>>> + Send;
    fn update_value(&self, user_id: i64, target: UpdateTarget) -> impl Future<Output = Result<(), RepoError<TypeConversionError>>> + Send;
    fn activate_premium(&self, user_id: i64, variant: PremiumVariant) -> impl Future<Output = Result<Option<DateTime<Utc>>, RepoError<TypeConversionError>>> + Send;
}

#[derive(Clone, Constructor)]
pub struct UsersPostgres {
    pool: sqlx::Pool<sqlx::Postgres>
}

impl Users for UsersPostgres {
    #[tracing::instrument(skip(self), fields(user_id = %id))]
    async fn get(&self, id: UserId) -> Result<Option<SavedUser>, RepoError<TypeConversionError>> {
        let result = match id {
            UserId::Internal(id) => {
                tracing::debug!(id, "Fetching user by internal ID");
                Self::get_user_internal(&self.pool, id).await
            }
            UserId::External(external_id) => {
                tracing::debug!(external_id, "Fetching user by external ID");
                sqlx::query_as!(UserInternal,
                    "SELECT id, name, language_code, location, premium_till FROM Users u
                    JOIN User_Service_Mappings usm ON u.id = usm.user_id
                    WHERE external_id = $1", external_id)
                .fetch_optional(&self.pool)
                .await
            }
        };
        match result {
            Ok(Some(user)) => {
                tracing::debug!("User found in database");
                user.try_into().map_err(RepoError::Other).map(Some)
            }
            Ok(None) => {
                tracing::debug!("User not found in database");
                Ok(None)
            }
            Err(e) => {
                tracing::error!(error = %e, "Database error while fetching user");
                Err(e.into())
            }
        }
    }

    #[tracing::instrument(skip(self, user, consent_info), fields(external_id = %user.external_id, service_id = %service_id))]
    async fn register(&self, user: ExternalUser, service_id: i32, consent_info: serde_json::Value) -> Result<i64, RepoError<TypeConversionError>> {
        tracing::debug!("Starting user registration transaction");
        let mut tx = self.pool.begin().await?;

        tracing::debug!("Inserting user into Users table");
        let user_id = sqlx::query_scalar!("INSERT INTO Users (name) VALUES ($1) RETURNING id",
                user.name)
            .fetch_one(&mut *tx)
            .await?;
        tracing::debug!(user_id, "User created with ID");

        tracing::debug!("Inserting user-service mapping");
        let rows = sqlx::query!(
            "INSERT INTO User_Service_Mappings (user_id, service_id, external_id)
             VALUES ($1, $2, $3) ON CONFLICT (service_id, external_id) DO NOTHING",
            user_id, service_id, user.external_id
        )
            .execute(&mut *tx)
            .await?;

        if rows.rows_affected() == 0 {
            tracing::debug!(orphan_user_id = %user_id, "Concurrent registration detected, rolling back orphan");
            tx.rollback().await?;
            return Ok(sqlx::query_scalar!(
                "SELECT user_id FROM User_Service_Mappings
                 WHERE service_id = $1 AND external_id = $2",
                service_id, user.external_id
            )
                .fetch_one(&self.pool)
                .await?);
        }

        tracing::debug!("Inserting consent information");
        sqlx::query!("INSERT INTO Consents (uid, service_id, info) VALUES ($1, $2, $3)",
                user_id, service_id, consent_info)
            .execute(&mut *tx)
            .await?;

        tracing::debug!("Committing transaction");
        tx.commit().await?;
        tracing::info!(user_id, "User registered successfully");
        Ok(user_id)
    }

    #[tracing::instrument(skip(self), fields(service_id = %service_id, external_id = %external_id))]
    async fn get_user_id(&self, service_id: i32, external_id: i64) -> Result<Option<i64>, RepoError<TypeConversionError>> {
        tracing::debug!("Querying user ID by service and external ID");
        let result = sqlx::query_scalar!("SELECT user_id FROM User_Service_Mappings
                WHERE service_id = $1 AND external_id = $2",
                service_id, external_id)
            .fetch_optional(&self.pool)
            .await?;
        if let Some(user_id) = result {
            tracing::debug!(user_id, "Found user ID");
        } else {
            tracing::debug!("No user ID found");
        }
        Ok(result)
    }

    #[tracing::instrument(skip(self), fields(user_id = %user_id, update_target = ?target))]
    async fn update_value(&self, user_id: i64, target: UpdateTarget) -> Result<(), RepoError<TypeConversionError>> {
        tracing::debug!("Updating user value");
        let rows_affected = match target {
            UpdateTarget::Language(code) => self.update_language(user_id, code).await,
            UpdateTarget::Location { latitude, longitude } => self.update_location(user_id, latitude, longitude).await,
        }?.rows_affected();

        if rows_affected.is_zero() {
            tracing::warn!("No rows affected - user not found");
            Err(sqlx::Error::RowNotFound.into())
        } else {
            tracing::info!(rows_affected, "User value updated successfully");
            Ok(())
        }
    }

    #[tracing::instrument(skip(self), fields(user_id = %user_id, variant = ?variant))]
    async fn activate_premium(&self, user_id: i64, variant: PremiumVariant) -> Result<Option<DateTime<Utc>>, RepoError<TypeConversionError>> {
        tracing::debug!("Fetching current premium status");
        let Some(current_premium_till) = sqlx::query!(
            "SELECT premium_till FROM Users WHERE id = $1",
            user_id
        )
            .fetch_optional(&self.pool)
            .await?
            .map(|row| row.premium_till)
        else {
            tracing::warn!("User not found");
            return Ok(None);
        };

        let start_datetime = current_premium_till.unwrap_or_else(|| {
            tracing::debug!("No existing premium - starting from now");
            Utc::now()
        });
        let till = variant + start_datetime;
        tracing::debug!(premium_till = %till, "Calculated new premium expiry");

        let rows_affected = sqlx::query!(
            "UPDATE Users SET premium_till = $2
             WHERE id = $1 AND premium_till IS NOT DISTINCT FROM $3",
            user_id, till, current_premium_till
        ).execute(&self.pool).await?.rows_affected();

        if rows_affected == 0 {
            tracing::warn!("Concurrent premium activation detected");
            return Err(sqlx::Error::RowNotFound.into());
        }

        tracing::info!(premium_till = %till, "Premium activated successfully");
        Ok(Some(till))
    }
}

impl UsersPostgres {
    async fn update_language(&self, user_id: i64, language: Code) -> Result<PgQueryResult, sqlx::Error> {
        tracing::debug!(?language, "Updating language");
        let lang_code: String = language.into();
        sqlx::query!("UPDATE Users SET language_code = $2 WHERE id = $1", user_id, lang_code)
            .execute(&self.pool)
            .await
    }

    async fn update_location(&self, user_id: i64, latitude: f64, longitude: f64) -> Result<PgQueryResult, sqlx::Error> {
        tracing::debug!(latitude, longitude, "Updating location");
        sqlx::query!("UPDATE Users SET location = ARRAY[$2::float8, $3::float8] WHERE id = $1", user_id, latitude, longitude)
            .execute(&self.pool)
            .await
    }

    async fn get_user_internal<'a, E>(executor: E, id: i64) -> Result<Option<UserInternal>, sqlx::Error>
    where E: sqlx::Executor<'a, Database = sqlx::Postgres>
    {
        sqlx::query_as!(UserInternal,
                    "SELECT id, name, language_code, location, premium_till FROM Users WHERE id = $1", id)
            .fetch_optional(executor)
            .await
    }
}
