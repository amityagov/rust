mod authenticated_user;
mod bearer_auth;
mod role;

use anyhow::Context;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use bearer_auth::BearerAuth;
use headers::Authorization;
use headers::authorization::Bearer;

use crate::error::ApplicationError;
pub use authenticated_user::AuthenticatedUser;
pub use role::RoleContainer;

#[derive(Clone)]
pub enum Authentication {
    Bearer(BearerAuth),
}

impl Authentication {
    pub fn bearer(aud: &str, iss: &str, jwk_base64: &str) -> anyhow::Result<Self> {
        Ok(Authentication::Bearer(BearerAuth::new(
            aud, iss, jwk_base64,
        )?))
    }
}

impl Authentication {
    pub async fn authenticate<S>(
        &self,
        parts: &mut Parts,
        state: &S,
    ) -> Result<AuthenticatedUser, ApplicationError>
    where
        S: Send + Sync,
    {
        match self {
            Authentication::Bearer(auth) => {
                let TypedHeader::<Authorization<Bearer>>(bearer) =
                    TypedHeader::<_>::from_request_parts(parts, state)
                        .await
                        .context("Failed to get authorization header")
                        .map_err(|_| ApplicationError::Unauthorized)?;

                auth.authenticate(bearer.token())
            }
        }
    }
}
