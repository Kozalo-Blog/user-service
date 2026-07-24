use derive_more::{Display, From};
use thiserror::Error;
use crate::dto;
use crate::dto::error::{CodeStringLengthError, EnumUnspecifiedValue};
use crate::grpc::generated::update_user_request::Target;
use crate::grpc::generated::user::Options;
use crate::repo::users::UpdateTarget;

tonic::include_proto!("user_service");

/// Applies a `FieldMask`-style projection to a [`User`] in place, clearing any field whose path is
/// not selected. The internal `id` is always retained (correlation to the request is via the
/// `GetUsersResponse` map key, which holds the requested — possibly external — id).
///
/// Supported paths: `id`, `name`, `is_premium`, `options`, `options.language_code`,
/// `options.location`. Selecting a nested `options.*` path keeps `options` but drops its other
/// sub-fields; selecting bare `options` keeps the whole message. Unknown paths are ignored.
///
/// Note: `is_premium` is a bare proto3 `bool` with no field presence, so a masked-out `is_premium`
/// is reset to its default (`false`) rather than becoming truly absent — a client cannot tell
/// "pruned" from "not premium". This matches Google's `FieldMaskUtil` behaviour for scalar fields.
pub fn project_user(user: &mut User, paths: &[String]) {
    let keep_name = paths.iter().any(|p| p == "name");
    let keep_premium = paths.iter().any(|p| p == "is_premium");
    let keep_options = paths.iter().any(|p| p == "options");
    let keep_language = keep_options || paths.iter().any(|p| p == "options.language_code");
    let keep_location = keep_options || paths.iter().any(|p| p == "options.location");

    if !keep_name {
        user.name = None;
    }
    if !keep_premium {
        user.is_premium = false;
    }
    if keep_language || keep_location {
        if let Some(options) = user.options.as_mut() {
            if !keep_language {
                options.language_code = None;
            }
            if !keep_location {
                options.location = None;
            }
        }
    } else {
        user.options = None;
    }
}

impl From<ExternalUser> for dto::ExternalUser {
    fn from(value: ExternalUser) -> Self {
        dto::ExternalUser {
            external_id: value.external_id,
            name: value.name,
        }
    }
}

impl From<dto::SavedUser> for User {
    fn from(value: dto::SavedUser) -> Self {
        let is_premium = value.premium();
        Self {
            id: value.id,
            name: value.name,
            options: Some(Options {
                language_code: value.language_code.map(Into::into),
                location: value.location.map(Into::into),
            }),
            is_premium,
        }
    }
}

impl From<dto::Location> for Location {
    fn from(value: dto::Location) -> Self {
        Self {
            latitude: value.latitude,
            longitude: value.longitude,
        }
    }
}

#[derive(Debug, Error, Display)]
pub struct UnspecifiedServiceType;

impl TryInto<dto::ServiceType> for ServiceType {
    type Error = UnspecifiedServiceType;

    fn try_into(self) -> Result<dto::ServiceType, Self::Error> {
        match self {
            ServiceType::Unspecified => Err(UnspecifiedServiceType),
            ServiceType::TelegramBot => Ok(dto::ServiceType::TelegramBot),
            ServiceType::TelegramChannel => Ok(dto::ServiceType::TelegramChannel),
            ServiceType::Website => Ok(dto::ServiceType::Website),
            ServiceType::Application => Ok(dto::ServiceType::Application),
        }
    }
}

#[derive(Debug, Error, Display, From)]
pub enum TargetConversionError {
    LanguageCodeConversionError(CodeStringLengthError),
}

impl TryInto<UpdateTarget> for Target {
    type Error = TargetConversionError;

    fn try_into(self) -> Result<UpdateTarget, Self::Error> {
        let target: UpdateTarget = match self {
            Target::Language(code) => dto::Code::try_from(code)?.into(),
            Target::Location(loc) => (loc.latitude, loc.longitude).into(),
        };
        Ok(target)
    }
}

impl From<dto::RegistrationResponse> for RegistrationResponse {
    fn from(value: dto::RegistrationResponse) -> Self {
        let grpc_status: RegistrationStatus = value.status.into();
        Self {
            status: grpc_status.into(),
            id: value.id,
        }
    }
}

impl From<dto::RegistrationStatus> for RegistrationStatus {
    fn from(value: dto::RegistrationStatus) -> Self {
        match value {
            dto::RegistrationStatus::Created => Self::Created,
            dto::RegistrationStatus::AlreadyPresent => Self::AlreadyPresent,
        }
    }
}

impl TryInto<dto::PremiumVariant> for PremiumVariant {
    type Error = EnumUnspecifiedValue;

    fn try_into(self) -> Result<dto::PremiumVariant, Self::Error> {
        match self {
            Self::Unspecified => Err(EnumUnspecifiedValue),
            Self::Month => Ok(dto::PremiumVariant::Month),
            Self::Quarter => Ok(dto::PremiumVariant::Quarter),
            Self::HalfYear => Ok(dto::PremiumVariant::HalfYear),
            Self::Year => Ok(dto::PremiumVariant::Year),
        }
    }
}
