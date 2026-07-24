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

/// Query parameters for `GET /search`. Internal and external ids are named explicitly and may be
/// mixed in one call (serde_urlencoded has no repeated-key `Vec` support, so each is comma-separated).
#[derive(Deserialize)]
pub struct SearchParams {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub external_id: Option<String>,
    /// Comma-separated `FieldMask` paths, e.g. `options.language_code`.
    #[serde(default)]
    pub fields: Option<String>,
}

#[derive(Clone, FromStr)]
pub enum PremiumVariantRest {
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl From<PremiumVariantRest> for PremiumVariant {
    fn from(value: PremiumVariantRest) -> Self {
        match value {
            PremiumVariantRest::Month => PremiumVariant::Month,
            PremiumVariantRest::Quarter => PremiumVariant::Quarter,
            PremiumVariantRest::HalfYear => PremiumVariant::HalfYear,
            PremiumVariantRest::Year => PremiumVariant::Year,
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
