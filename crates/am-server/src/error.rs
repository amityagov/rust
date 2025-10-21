use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug)]
pub enum ApplicationError {
    InternalServerError,
    NotFound,
    Unauthorized,
    Forbidden,
}

#[derive(serde::Serialize)]
struct ErrorResponse {
    message: String,
}

impl IntoResponse for ApplicationError {
    fn into_response(self) -> Response {
        match self {
            ApplicationError::InternalServerError => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    message: "Internal Server Error".to_string(),
                }),
            ),
            ApplicationError::NotFound => (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    message: "Not Found".to_string(),
                }),
            ),
            ApplicationError::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    message: "Unauthorized".to_string(),
                }),
            ),
            ApplicationError::Forbidden => (
                StatusCode::FORBIDDEN,
                Json(ErrorResponse {
                    message: "Forbidden ".to_string(),
                }),
            ),
        }
        .into_response()
    }
}

pub fn system_error() -> ApplicationError {
    ApplicationError::InternalServerError
}

pub fn not_found() -> ApplicationError {
    ApplicationError::NotFound
}
