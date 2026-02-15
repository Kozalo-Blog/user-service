use axum_route_error::RouteError;
use crate::rest::RestError;

/// Extension trait for REST-specific error logging and conversion
pub trait RestErrorExt<T> {
    /// Log error at ERROR level and convert to internal server error
    fn log_route_error(self, message: &str) -> Result<T, RouteError<RestError>>;

    /// Log error at WARN level and convert to bad request error
    fn log_route_warn(self, message: &str) -> Result<T, RouteError<RestError>>;
}

impl<T, E> RestErrorExt<T> for Result<T, E>
where
    E: std::fmt::Display + Into<RestError>,
{
    fn log_route_error(self, message: &str) -> Result<T, RouteError<RestError>> {
        self.map_err(|e| {
            tracing::error!(error = %e, "{}", message);
            RouteError::new_internal_server().set_error_data(e.into())
        })
    }

    fn log_route_warn(self, message: &str) -> Result<T, RouteError<RestError>> {
        self.map_err(|e| {
            tracing::warn!(error = %e, "{}", message);
            RouteError::new_bad_request().set_error_data(e.into())
        })
    }
}
