mod user;

pub use user::*;

use axum::response::{IntoResponse, Response};
use serde_derive::Deserialize;
use serde_json::json;
use crate::dto::{ExternalUser, Service};

#[derive(Deserialize)]
pub struct RegistrationRequest {
    pub user: ExternalUser,
    pub service: Service,
}

pub struct Success;

impl IntoResponse for Success {
    fn into_response(self) -> Response {
        json!({"success": true}).to_string().into_response()
    }
}
