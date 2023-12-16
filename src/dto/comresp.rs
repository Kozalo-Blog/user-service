use serde_derive::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub status: RegistrationStatus,
    pub id: i64,
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
