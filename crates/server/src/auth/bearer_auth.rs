use crate::auth::AuthenticatedUser;
use crate::error::ApplicationError;
use anyhow::Context;
use base64::Engine;
use jsonwebtoken::Algorithm::ES256;
use jsonwebtoken::{DecodingKey, TokenData, Validation};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use std::sync::Arc;

#[derive(Clone)]
pub struct BearerAuth {
    inner: Arc<Inner>,
}

pub struct Inner {
    validation: Validation,
    decoding_key: jsonwebtoken::DecodingKey,
}

impl Deref for BearerAuth {
    type Target = Inner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    sub: String,
    aud: String,
    iss: String,
    scope: String,
    exp: usize,
}

impl BearerAuth {
    pub fn new(aud: &str, iss: &str, jwk_base64: &str) -> anyhow::Result<Self> {
        let mut validation = Validation::new(ES256);
        validation.validate_exp = true;
        validation.validate_aud = true;
        validation.aud = Some([aud.to_string()].into());
        validation.iss = Some([iss.to_string()].into());

        let bytes = base64::engine::general_purpose::URL_SAFE
            .decode(&jwk_base64)
            .context("Failed to decode JWK bytes from base64")?;

        // decoding key used for validation
        let decoding_key = DecodingKey::from_ec_pem(&bytes).context("Failed to decode JWK key")?;

        Ok(Self {
            inner: Arc::new(Inner {
                validation,
                decoding_key,
            }),
        })
    }

    pub fn authenticate(&self, token: &str) -> Result<AuthenticatedUser, ApplicationError> {
        let TokenData { header: _, claims }: TokenData<Claims> =
            jsonwebtoken::decode(token, &self.decoding_key, &self.validation)
                .context("Invalid token")
                .map_err(|_| ApplicationError::Unauthorized)?;

        Ok(AuthenticatedUser {
            id: claims.sub,
            role: None,
        })
    }
}
