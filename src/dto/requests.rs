use axum::response::{IntoResponse, Response};
use serde::{Serialize, Deserialize};
use serde_json::json;
use crate::dto::service::Service;
use crate::dto::user::ExternalUser;

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

#[derive(Serialize, Deserialize)]
pub struct RegistrationResponse {
    status: RegistrationStatus,
    id: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RegistrationStatus {
    Created,
    AlreadyPresent,
}

impl RegistrationStatus {
    pub fn with_id(self, id: i64) -> RegistrationResponse {
        RegistrationResponse {
            id,
            status: self
        }
    }
}
