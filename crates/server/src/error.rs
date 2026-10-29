use std::borrow::Cow;

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

/// An error that turns into a JSON response `{"code": ..., "message": ...}`.
///
/// The unit variants are the stock errors with fixed messages. Use [`ApplicationError::new`]
/// (or one of the status helpers such as [`ApplicationError::conflict`]) to return a custom
/// message together with a stable, machine-readable `code` that clients can branch on.
#[derive(Debug)]
pub enum ApplicationError {
    InternalServerError,
    NotFound,
    Unauthorized,
    Forbidden,
    /// An error with a caller-provided status, code and message.
    Custom {
        status: StatusCode,
        code: Cow<'static, str>,
        message: Cow<'static, str>,
    },
}

#[derive(serde::Serialize)]
struct ErrorResponse<'a> {
    code: &'a str,
    message: &'a str,
}

impl ApplicationError {
    /// Builds an error with an arbitrary status, code and message.
    pub fn new(
        status: StatusCode,
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::Custom {
            status,
            code: code.into(),
            message: message.into(),
        }
    }

    /// 400: the request itself is malformed.
    pub fn bad_request(
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    /// 401: missing or rejected credentials.
    pub fn unauthorized(
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, message)
    }

    /// 403: authenticated, but not allowed.
    pub fn forbidden(
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::FORBIDDEN, code, message)
    }

    /// 404: the addressed resource does not exist.
    pub fn not_found(
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::NOT_FOUND, code, message)
    }

    /// 409: the request is valid but conflicts with the current state.
    pub fn conflict(
        code: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Self {
        Self::new(StatusCode::CONFLICT, code, message)
    }

    /// 500 with the generic message; put the cause in the log, not in the response.
    pub fn internal() -> Self {
        Self::InternalServerError
    }

    /// The HTTP status of this error.
    pub fn status(&self) -> StatusCode {
        match self {
            Self::InternalServerError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::Custom { status, .. } => *status,
        }
    }

    /// The machine-readable code of this error.
    pub fn code(&self) -> &str {
        match self {
            Self::InternalServerError => "internal_error",
            Self::NotFound => "not_found",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Custom { code, .. } => code,
        }
    }

    /// The human-readable message of this error.
    pub fn message(&self) -> &str {
        match self {
            Self::InternalServerError => "Internal Server Error",
            Self::NotFound => "Not Found",
            Self::Unauthorized => "Unauthorized",
            Self::Forbidden => "Forbidden",
            Self::Custom { message, .. } => message,
        }
    }
}

impl std::fmt::Display for ApplicationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code(), self.message())
    }
}

impl std::error::Error for ApplicationError {}

impl IntoResponse for ApplicationError {
    fn into_response(self) -> Response {
        (
            self.status(),
            Json(ErrorResponse {
                code: self.code(),
                message: self.message(),
            }),
        )
            .into_response()
    }
}

pub fn system_error() -> ApplicationError {
    ApplicationError::InternalServerError
}

pub fn not_found() -> ApplicationError {
    ApplicationError::NotFound
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;

    async fn body_of(error: ApplicationError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        (status, serde_json::from_slice(&bytes).unwrap())
    }

    #[tokio::test]
    async fn stock_errors_keep_their_messages_and_gain_a_code() {
        let (status, body) = body_of(ApplicationError::NotFound).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["message"], "Not Found");
        assert_eq!(body["code"], "not_found");
    }

    #[tokio::test]
    async fn custom_errors_carry_status_code_and_message() {
        let (status, body) =
            body_of(ApplicationError::conflict("keys_missing", "No keys yet.")).await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["code"], "keys_missing");
        assert_eq!(body["message"], "No keys yet.");
    }
}
