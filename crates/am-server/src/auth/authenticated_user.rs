use crate::auth::Authentication;
use crate::error::ApplicationError;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;

#[derive(Clone)]
pub struct AuthenticatedUser {
    pub id: String,
    pub role: Option<String>,
}

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = ApplicationError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let authorization = parts
            .extensions
            .get::<Authentication>()
            .cloned()
            .ok_or(ApplicationError::Unauthorized)?;

        authorization.authenticate(parts, state).await
    }
}
