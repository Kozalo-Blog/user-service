use chrono::{DateTime, Months, Utc};
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

pub trait PremiumVariants {
    fn get_months(&self) -> u32;

    fn to_datetime(&self) -> DateTime<Utc> {
        let months = Months::new(self.get_months());
        Utc::now()
            .checked_add_months(months)
            .expect("something very bad was happened: the date, till the premium subscription will be active, is out of range O_o")
    }
}
