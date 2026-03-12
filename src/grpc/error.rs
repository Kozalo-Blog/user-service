use tonic::Status;

/// Extension trait for converting Results into gRPC Status with automatic tracing
pub trait IntoStatusExt<T> {
    /// Convert error into Status::internal with error-level logging
    fn into_status(self) -> Result<T, Status>;

    /// Convert error into Status::invalid_argument with warn-level logging
    fn into_invalid_argument(self) -> Result<T, Status>;
}

impl<T, E: std::fmt::Display> IntoStatusExt<T> for Result<T, E> {
    fn into_status(self) -> Result<T, Status> {
        self.map_err(|e| {
            tracing::error!(error = %e, "Internal error");
            Status::internal(e.to_string())
        })
    }

    fn into_invalid_argument(self) -> Result<T, Status> {
        self.map_err(|e| {
            tracing::warn!(error = %e, "Invalid argument");
            Status::invalid_argument(e.to_string())
        })
    }
}

/// Extension trait for Option to convert into gRPC Status with logging
pub trait IntoStatusOptionExt<T> {
    /// Convert None into Status::not_found with warn-level logging
    fn ok_or_not_found(self, message: &str) -> Result<T, Status>;

    /// Convert None into Status::invalid_argument with warn-level logging
    fn ok_or_invalid_argument(self, message: &str) -> Result<T, Status>;
}

impl<T> IntoStatusOptionExt<T> for Option<T> {
    fn ok_or_not_found(self, message: &str) -> Result<T, Status> {
        self.ok_or_else(|| {
            tracing::warn!(message = %message, "Resource not found");
            Status::not_found(message)
        })
    }

    fn ok_or_invalid_argument(self, message: &str) -> Result<T, Status> {
        self.ok_or_else(|| {
            tracing::warn!(message = %message, "Invalid argument");
            Status::invalid_argument(message)
        })
    }
}
