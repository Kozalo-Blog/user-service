use std::sync::Arc;
use std::time::SystemTime;
use autometrics::autometrics;
use derive_more::Constructor;
use tonic::{Request, Response, Status};
use crate::grpc::generated::user_service_server::UserService;
use crate::grpc::generated::{ActivatePremiumRequest, ActivatePremiumResponse, GetUserRequest, PremiumVariant, RegistrationRequest, RegistrationResponse, ServiceType, UpdateUserRequest, User};
use crate::dto::RegistrationStatus;
use crate::{dto, repo};
use crate::repo::users::{UserId, Users};
use crate::repo::services::Services;
use crate::grpc::error::{IntoStatusExt, IntoStatusOptionExt};

#[derive(Constructor)]
pub struct GrpcServer<U, S>
where
    U: Users,
    S: Services,
{
    repos: Arc<repo::Repositories<U, S>>
}

#[tonic::async_trait]
impl<U, S> UserService for GrpcServer<U, S>
where
    U: Users + Send + Sync + 'static,
    S: Services + Send + Sync + 'static,
{
    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id, by_external_id = %request.get_ref().by_external_id))]
    #[autometrics]
    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
        let req = request.into_inner();
        let id = if req.by_external_id {
            UserId::External(req.id)
        } else {
            UserId::Internal(req.id)
        };
        let user = self.repos.users.get(id).await
            .into_status()?
            .map(Into::into)
            .ok_or_not_found("The user is not found")?;
        Ok(Response::new(user))
    }

    #[tracing::instrument(skip(self, request), fields(
        external_id = request.get_ref().user.as_ref().map(|u| u.external_id).unwrap_or(0),
        service_name = request.get_ref().service.as_ref().map(|s| s.name.as_str()).unwrap_or("")
    ))]
    #[autometrics]
    async fn register(&self, request: Request<RegistrationRequest>) -> Result<Response<RegistrationResponse>, Status> {
        let req = request.into_inner();

        let (service_name, service_type_id) = req.service
            .map(|s| (s.name, s.kind))
            .ok_or_invalid_argument("The 'service' field is not set")?;
        let grpc_service_type: ServiceType = service_type_id.try_into()
            .into_invalid_argument()?;
        let service_type: dto::ServiceType = grpc_service_type.try_into()
            .into_invalid_argument()?;
        let service = (service_name.clone(), service_type).into();

        let maybe_service_id = self.repos.services.get_id(&service).await
            .into_status()?;
        let service_id = match maybe_service_id {
            None => self.repos.services.create(service_type, &service_name).await
                .into_status()?,
            Some(id) => id
        };

        let external_user: dto::ExternalUser = req.user
            .map(|ext_usr| ext_usr.into())
            .ok_or_invalid_argument("The 'user' field is not set")?;
        let maybe_user_id = self.repos.users.get_user_id(service_id, external_user.external_id).await
            .into_status()?;

        let resp = match maybe_user_id {
            None => {
                let consent_info = req.consent_info
                    .and_then(|info| serde_json::to_value(info).ok())
                    .ok_or_invalid_argument("The 'consent_info' field is not set or invalid")?;
                let id = self.repos.users.register(external_user, service_id, consent_info).await
                    .into_status()?;
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

    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id))]
    #[autometrics]
    async fn update(&self, request: Request<UpdateUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let grpc_target = req.target
            .ok_or_invalid_argument("The 'target' field is not set")?;
        let repo_target = grpc_target.try_into()
            .into_invalid_argument()?;
        self.repos.users.update_value(req.id, repo_target).await
            .into_status()?;
        tracing::info!("User updated successfully");
        Ok(Response::new(()))
    }

    #[tracing::instrument(skip(self, request), fields(user_id = %request.get_ref().id, variant = %request.get_ref().variant))]
    #[autometrics]
    async fn activate_premium(&self, request: Request<ActivatePremiumRequest>) -> Result<Response<ActivatePremiumResponse>, Status> {
        let req = request.into_inner();
        let grpc_variant = PremiumVariant::try_from(req.variant)
            .into_invalid_argument()?;
        let variant = grpc_variant.try_into()
            .into_invalid_argument()?;
        let updated = self.repos.users.activate_premium(req.id, variant).await
            .into_status()?;

        let response = updated
            .inspect(|till| tracing::info!(active_till = %till, "Premium activated successfully"))
            .map(|till| ActivatePremiumResponse {
                updated: true,
                active_till: Some(SystemTime::from(till).into()),
            })
            .unwrap_or_else(|| {
                tracing::warn!("Premium activation failed - user not found");
                ActivatePremiumResponse {
                    updated: false,
                    active_till: None,
                }
            });
        Ok(Response::new(response))
    }
}
