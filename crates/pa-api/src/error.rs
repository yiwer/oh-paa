use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use pa_core::AppError;
use serde_json::json;

pub(crate) type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
pub(crate) struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }
}

impl From<AppError> for ApiError {
    fn from(value: AppError) -> Self {
        let status = match value {
            AppError::Validation { .. } => StatusCode::BAD_REQUEST,
            AppError::Provider { .. } | AppError::Storage { .. } | AppError::Analysis { .. } => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        };

        Self {
            status,
            message: value.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "error": self.message }))).into_response()
    }
}
