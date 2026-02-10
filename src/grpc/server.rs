use std::sync::Arc;
use std::time::SystemTime;
use autometrics::autometrics;
use derive_more::Constructor;
use tonic::{Request, Response, Status};
use crate::grpc::generated::user_service_server::UserService;
use crate::grpc::generated::{ActivatePremiumRequest, ActivatePremiumResponse, GetUserRequest, PremiumVariant, RegistrationRequest, RegistrationResponse, ServiceType, UpdateUserRequest, User};
use crate::dto::RegistrationStatus;
use crate::{dto, repo};
use crate::dto::error::EnumUnspecifiedValue;
use crate::grpc::generated::{TargetConversionError, UnspecifiedServiceType};
use crate::repo::users::UserId;

#[derive(Constructor)]
pub struct GrpcServer {
    repos: Arc<repo::Repositories>
}

#[tonic::async_trait]
impl UserService for GrpcServer {
    #[autometrics]
    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id, by_external_id = %request.get_ref().by_external_id))]
    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
        let req = request.into_inner();
        let id = if req.by_external_id {
            UserId::External(req.id)
        } else {
            UserId::Internal(req.id)
        };
        let user = self.repos.users.get(id).await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get user");
                Status::internal(e.to_string())
            })?
            .map(Into::into)
            .ok_or_else(|| {
                tracing::warn!("User not found");
                Status::not_found("The user is not found")
            })?;
        Ok(Response::new(user))
    }

    #[autometrics]
    #[tracing::instrument(skip(self, request), fields(
        external_id = request.get_ref().user.as_ref().map(|u| u.external_id).unwrap_or(0),
        service_name = request.get_ref().service.as_ref().map(|s| s.name.as_str()).unwrap_or("")
    ))]
    async fn register(&self, request: Request<RegistrationRequest>) -> Result<Response<RegistrationResponse>, Status> {
        let req = request.into_inner();

        let (service_name, service_type_id) = req.service.map(|s| (s.name, s.kind))
            .ok_or_else(|| {
                tracing::warn!("Service field not set in request");
                Status::invalid_argument("The 'service' field is not set")
            })?;
        let grpc_service_type: ServiceType = service_type_id.try_into()
            .map_err(|e| {
                tracing::warn!("Invalid service type: {:?}", e);
                Status::invalid_argument(format!("Invalid service type: {:?}", e))
            })?;
        let service_type: dto::ServiceType = grpc_service_type.try_into()
            .map_err(|e: UnspecifiedServiceType| {
                tracing::warn!(error = %e, "Unspecified service type");
                Status::invalid_argument(e.to_string())
            })?;
        let service = (service_name.clone(), service_type).into();

        let maybe_service_id = self.repos.services.get_id(&service).await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get service ID");
                Status::internal(e.to_string())
            })?;
        let service_id = match maybe_service_id {
            None => self.repos.services.create(service_type, &service_name).await
                .map_err(|e| {
                    tracing::error!(error = %e, "Failed to create service");
                    Status::internal(e.to_string())
                })?,
            Some(id) => id
        };

        let external_user: dto::ExternalUser = req.user
            .map(|ext_usr| ext_usr.into())
            .ok_or_else(|| {
                tracing::warn!("User field not set in request");
                Status::not_found("The 'user' field is not set")
            })?;
        let maybe_user_id = self.repos.users.get_user_id(service_id, external_user.external_id).await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to get user ID");
                Status::internal(e.to_string())
            })?;

        let resp = match maybe_user_id {
            None => {
                let consent_info = req.consent_info
                    .and_then(|info| serde_json::to_value(info).ok())
                    .ok_or_else(|| {
                        tracing::warn!("Consent info not set or invalid");
                        Status::invalid_argument("The 'consent_info' field is not set or invalid")
                    })?;
                let id = self.repos.users.register(external_user, service_id, consent_info).await
                    .map_err(|e| {
                        tracing::error!(error = %e, "Failed to register user");
                        Status::internal(e.to_string())
                    })?;
                tracing::info!(user_id = %id, "User registered successfully");
                RegistrationStatus::Created.with_id(id)
            }
            Some(id) => {
                tracing::info!(user_id = %id, "User already registered");
                RegistrationStatus::AlreadyPresent.with_id(id)
            }
        };
        Ok(Response::new(resp.into()))
    }

    #[autometrics]
    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id))]
    async fn update(&self, request: Request<UpdateUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let grpc_target = req.target
            .ok_or_else(|| {
                tracing::warn!("Target field not set in request");
                Status::invalid_argument("The 'target' field is not set")
            })?;
        let repo_target = grpc_target.try_into()
            .map_err(|e: TargetConversionError| {
                tracing::warn!(error = %e, "Invalid target conversion");
                Status::invalid_argument(e.to_string())
            })?;
        self.repos.users.update_value(req.id, repo_target).await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to update user");
                Status::internal(e.to_string())
            })?;
        tracing::info!("User updated successfully");
        Ok(Response::new(()))
    }

    #[autometrics]
    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id, variant = %request.get_ref().variant))]
    async fn activate_premium(&self, request: Request<ActivatePremiumRequest>) -> Result<Response<ActivatePremiumResponse>, Status> {
        let req = request.into_inner();
        let grpc_variant = PremiumVariant::try_from(req.variant)
            .map_err(|e| {
                tracing::warn!(error = %e, "Invalid premium variant");
                Status::invalid_argument(e.to_string())
            })?;
        let variant = grpc_variant.try_into()
            .map_err(|e: EnumUnspecifiedValue| {
                tracing::warn!(error = %e, "Unspecified premium variant");
                Status::invalid_argument(e.to_string())
            })?;
        let updated= self.repos.users.activate_premium(req.id, variant).await
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to activate premium");
                Status::internal(e.to_string())
            })?;

        let response = match updated {
            None => {
                tracing::warn!("Premium activation failed - user not found");
                ActivatePremiumResponse {
                    updated: false,
                    active_till: None,
                }
            }
            Some(till) => {
                tracing::info!(active_till = %till, "Premium activated successfully");
                let system_time = SystemTime::from(till);
                let timestamp = system_time.into();
                ActivatePremiumResponse {
                    updated: true,
                    active_till: Some(timestamp),
                }
            }
        };
        Ok(Response::new(response))
    }
}
