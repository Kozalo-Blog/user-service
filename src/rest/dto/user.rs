use serde_derive::{Deserialize, Serialize};
use crate::dto::{Location, SavedUser};

/// DTO for JSON response
#[derive(Serialize, Deserialize)]
pub struct UserView {
    id: i64,
    name: Option<String>,
    options: Options,
    is_premium: bool
}

#[derive(Serialize, Deserialize)]
pub struct Options {
    pub language_code: Option<String>,
    pub location: Option<Location>,
}


// IMPLEMENTATIONS


impl From<SavedUser> for UserView {
    fn from(value: SavedUser) -> Self {
        let is_premium = value.premium();
        Self {
            id: value.id,
            name: value.name,
            options: Options {
                language_code: value.language_code.map(Into::into),
                location: value.location.map(Into::into),
            },
            is_premium,
        }
    }
}
