use std::sync::Arc;
use autometrics::autometrics;
use derive_more::Constructor;
use prost::DecodeError;
use tonic::{Request, Response, Status};
use crate::grpc::generated::user_service_server::UserService;
use crate::grpc::generated::{GetUserRequest, RegistrationRequest, RegistrationResponse, ServiceType, UpdateUserRequest, User};
use crate::dto::RegistrationStatus;
use crate::{dto, repo};
use crate::grpc::generated::{TargetConversionError, UnspecifiedServiceType};
use crate::repo::users::UserId;

#[derive(Constructor)]
pub struct GrpcServer {
    repos: Arc<repo::Repositories>
}

#[tonic::async_trait]
impl UserService for GrpcServer {
    #[autometrics]
    async fn get(&self, request: Request<GetUserRequest>) -> Result<Response<User>, Status> {
        let req = request.into_inner();
        let id = if req.by_external_id {
            UserId::External(req.id)
        } else {
            UserId::Internal(req.id)
        };
        let user = self.repos.users.get(id).await
            .map_err(|e| Status::internal(e.to_string()))?
            .map(Into::into)
            .ok_or(Status::not_found("The user is not found"))?;
        Ok(Response::new(user))
    }

    #[autometrics]
    async fn register(&self, request: Request<RegistrationRequest>) -> Result<Response<RegistrationResponse>, Status> {
        let req = request.into_inner();

        let (service_name, service_type_id) = req.service.map(|s| (s.name, s.r#type))
            .ok_or(Status::invalid_argument("The 'service' field is not set"))?;
        let grpc_service_type: ServiceType = service_type_id.try_into()
            .map_err(|e: DecodeError| Status::invalid_argument(e.to_string()))?;
        let service_type: dto::ServiceType = grpc_service_type.try_into()
            .map_err(|e: UnspecifiedServiceType| Status::invalid_argument(e.to_string()))?;
        let service = (service_name.clone(), service_type).into();

        let maybe_service_id = self.repos.services.get_id(&service).await
            .map_err(|e| Status::internal(e.to_string()))?;
        let service_id = match maybe_service_id {
            None => self.repos.services.create(service_type, &service_name).await
                .map_err(|e| Status::internal(e.to_string()))?,
            Some(id) => id
        };

        let external_user: dto::ExternalUser = req.user
            .map(|ext_usr| ext_usr.into())
            .ok_or(Status::not_found("The 'user' field is not set"))?;
        let maybe_user_id = self.repos.users.get_user_id(service_id, external_user.external_id).await
            .map_err(|e| Status::internal(e.to_string()))?;

        let resp = match maybe_user_id {
            None => {
                let id = self.repos.users.register(external_user, service_id).await
                    .map_err(|e| Status::internal(e.to_string()))?;
                RegistrationStatus::Created.with_id(id)
            }
            Some(id) => RegistrationStatus::AlreadyPresent.with_id(id),
        };
        Ok(Response::new(resp.into()))
    }

    #[autometrics]
    async fn update(&self, request: Request<UpdateUserRequest>) -> Result<Response<()>, Status> {
        let req = request.into_inner();
        let grpc_target = req.target
            .ok_or(Status::invalid_argument("The 'target' field is not set"))?;
        let repo_target = grpc_target.try_into()
            .map_err(|e: TargetConversionError| Status::invalid_argument(e.to_string()))?;
        self.repos.users.update_value(req.id, repo_target).await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(()))
    }
}
