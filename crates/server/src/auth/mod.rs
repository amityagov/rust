mod authenticated_user;
mod bearer_auth;
mod role;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::TypedHeader;
use bearer_auth::BearerAuth;
use headers::Authorization;
use headers::authorization::Bearer;

use crate::error::ApplicationError;
pub use authenticated_user::AuthenticatedUser;
pub use role::RoleContainer;

/// How incoming requests are authenticated. Install one in the request extensions (for example
/// with `axum::Extension`) so that the [`AuthenticatedUser`] extractor can find it.
#[derive(Clone)]
pub enum Authentication {
    Bearer(BearerAuth),
}

impl Authentication {
    /// Bearer-token authentication; see [`BearerAuth::new`] for the key format.
    pub fn bearer(aud: &str, iss: &str, public_key_pem_base64: &str) -> anyhow::Result<Self> {
        Ok(Authentication::Bearer(BearerAuth::new(
            aud,
            iss,
            public_key_pem_base64,
        )?))
    }

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
                    TypedHeader::from_request_parts(parts, state)
                        .await
                        .map_err(|_| ApplicationError::Unauthorized)?;

                auth.authenticate(bearer.token())
            }
        }
    }
}
