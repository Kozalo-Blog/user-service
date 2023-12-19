use derive_more::{Display, From};
use prost::DecodeError;
use thiserror::Error;
use crate::dto;
use crate::dto::error::CodeStringLengthError;
use crate::dto::PremiumVariants as PremiumVariantsTrait;
use crate::grpc::generated::update_user_request::{PremiumVariants, Target};
use crate::grpc::generated::user::Options;
use crate::repo::users::UpdateTarget;

tonic::include_proto!("grpc");

impl Into<dto::ExternalUser> for ExternalUser {
    fn into(self) -> dto::ExternalUser {
        dto::ExternalUser {
            external_id: self.external_id,
            name: self.name,
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
    PremiumActiveTillConversionError(DecodeError),
}

impl TryInto<UpdateTarget> for Target {
    type Error = TargetConversionError;

    fn try_into(self) -> Result<UpdateTarget, Self::Error> {
        let target: UpdateTarget = match self {
            Target::Language(code) => dto::Code::try_from(code)?.into(),
            Target::Location(loc) => (loc.latitude, loc.longitude).into(),
            Target::PremiumVariant(variant) => PremiumVariants::try_from(variant)?.to_datetime().into()
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

impl dto::PremiumVariants for PremiumVariants {
    fn get_months(&self) -> u32 {
        match self {
            PremiumVariants::Unspecified => {
                log::warn!("[gRPC] unspecified premium variant!");
                0
            },
            PremiumVariants::Month => 1,
            PremiumVariants::Quarter => 3,
            PremiumVariants::HalfYear => 6,
            PremiumVariants::Year => 12,
        }
    }
}
