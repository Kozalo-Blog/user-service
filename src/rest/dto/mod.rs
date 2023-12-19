mod user;

pub use user::*;

use axum::response::{IntoResponse, Response};
use derive_more::FromStr;
use serde_derive::{Deserialize, Serialize};
use serde_json::json;
use crate::dto::{ExternalUser, PremiumVariants, Service};

#[derive(Deserialize)]
pub struct RegistrationRequest {
    pub user: ExternalUser,
    pub service: Service,
}

#[derive(Clone, FromStr)]
pub enum PremiumVariantsRest {
    Month,
    Quarter,
    HalfYear,
    Year,
}

impl PremiumVariants for PremiumVariantsRest {
    fn get_months(&self) -> u32 {
        match self {
            Self::Month => 1,
            Self::Quarter => 3,
            Self::HalfYear => 6,
            Self::Year => 12,
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
