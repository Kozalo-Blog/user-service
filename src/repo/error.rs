use axum::Json;
use axum::response::{IntoResponse, Response};
use serde::{Serialize, Serializer};
use serde_derive::Deserialize;
use thiserror::Error;

#[derive(Debug, serde_derive::Serialize, Deserialize, Error, derive_more::Display)]
pub enum RepoError<EO: Serialize, EDB: Serialize = DatabaseError> {
    Database(EDB),
    Other(EO),
}

#[derive(Debug, Error, derive_more::Display)]
pub struct DatabaseError(pub sqlx::Error);

impl Serialize for DatabaseError {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.0.to_string().serialize(s)
    }
}

impl IntoResponse for DatabaseError {
    fn into_response(self) -> Response {
        Json(self).into_response()
    }
}

impl From<sqlx::Error> for DatabaseError {
    fn from(value: sqlx::Error) -> Self {
        Self(value)
    }
}
