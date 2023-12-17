use derive_more::From;
use serde_derive::{Deserialize, Serialize};

#[derive(sqlx::Type, Serialize, Deserialize, Debug, Hash, Eq, PartialEq, Copy, Clone)]
#[sqlx(type_name = "service_type")]
#[sqlx(rename_all = "kebab-case")]
#[serde(rename_all = "kebab-case")]
pub enum ServiceType {
    TelegramBot,
    TelegramChannel,
    Website,
    Application,
}

#[derive(Serialize, Deserialize, Clone, From, PartialEq, Eq)]
pub struct Service {
    pub name: String,

    #[serde(alias = "type")]
    pub service_type: ServiceType,
}
