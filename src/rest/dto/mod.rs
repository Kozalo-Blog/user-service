mod user;

pub use user::*;

use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use derive_more::FromStr;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use crate::dto::{ExternalUser, PremiumVariant, Service};

#[derive(Deserialize)]
pub struct RegistrationRequest {
    pub user: ExternalUser,
    pub service: Service,
    pub consent_info: serde_json::Value,
}

#[derive(Clone, FromStr)]
pub enum PremiumVariantRest {
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl Into<PremiumVariant> for PremiumVariantRest {
    fn into(self) -> PremiumVariant {
        match self {
            Self::Month => PremiumVariant::Month,
            Self::Quarter => PremiumVariant::Quarter,
            Self::HalfYear => PremiumVariant::HalfYear,
            Self::Year => PremiumVariant::Year,
        }
    }
}

#[derive(Serialize)]
pub struct PremiumActivationResult {
    success: bool,
    active_till: Option<DateTime<Utc>>,
}

impl From<Option<DateTime<Utc>>> for PremiumActivationResult {
    fn from(value: Option<DateTime<Utc>>) -> Self {
        Self {
            success: value.is_some(),
            active_till: value,
        }
    }
}

pub struct Success;

impl IntoResponse for Success {
    fn into_response(self) -> Response {
        axum::Json(json!({"success": true})).into_response()
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RestError {
    reason: String
}

impl <T: std::error::Error> From<T> for RestError {
    fn from(value: T) -> Self {
        Self { reason: value.to_string() }
    }
}
