use chrono::{DateTime, Utc};
use derive_more::From;
use serde_derive::{Deserialize, Serialize};
use crate::dto::error::{CodeStringLengthError, VecLengthAssertionError};

/// DTO for JSON request
#[derive(Serialize, Deserialize)]
pub struct ExternalUser {
    pub external_id: i64,
    pub name: Option<String>,
}

/// DTO for JSON response
#[derive(Serialize, Deserialize)]
pub struct UserView {
    id: i64,
    name: Option<String>,
    options: Options,
    is_premium: bool
}

/// Public DTO for the users fetched from the database.
/// See `crate::repo::users::UserInternal` to see the other, internal, side.
pub struct SavedUser {
    pub id: i64,
    pub name: Option<String>,
    pub language_code: Option<Code>,
    pub location: Option<Location>,
    pub premium_till: Option<DateTime<Utc>>
}

#[derive(Serialize, Deserialize)]
pub struct Options {
    language_code: Option<String>,
    location: Option<Location>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, From)]
pub struct Location {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub struct Code([char; 2]);


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

impl SavedUser {
    pub fn premium(&self) -> bool {
        self.premium_till
            .filter(|till| *till >= Utc::now())
            .is_some()
    }
}

impl From<[f64; 2]> for Location {
    fn from(value: [f64; 2]) -> Self {
        Self {
            latitude: value[0],
            longitude: value[1],
        }
    }
}

impl TryFrom<Vec<f64>> for Location {
    type Error = VecLengthAssertionError<f64>;

    fn try_from(value: Vec<f64>) -> Result<Self, Self::Error> {
        if value.len() == 2 {
            Ok([value[0], value[1]].into())
        } else {
            Err(VecLengthAssertionError::new(value, 2))
        }
    }
}

impl TryFrom<String> for Code {
    type Error = CodeStringLengthError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.as_str().try_into()
    }
}

impl TryFrom<&str> for Code {
    type Error = CodeStringLengthError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let chars: [char; 2] = value.chars()
            .collect::<Vec<char>>()
            .try_into()
            .map_err(|_| CodeStringLengthError)?;
        Ok(Self(chars))
    }
}

impl Into<String> for Code {
    fn into(self) -> String {
        format!("{}{}", self.0[0], self.0[1])
    }
}
