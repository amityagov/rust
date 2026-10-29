use crate::auth::AuthenticatedUser;
use crate::error::ApplicationError;
use anyhow::Context;
use base64::Engine;
use jsonwebtoken::Algorithm::ES256;
use jsonwebtoken::{DecodingKey, Validation};
use serde::Deserialize;
use std::sync::Arc;

/// Verifies ES256 JWTs against one public key, audience and issuer.
#[derive(Clone)]
pub struct BearerAuth {
    inner: Arc<Inner>,
}

struct Inner {
    validation: Validation,
    decoding_key: DecodingKey,
}

/// The claims every accepted token must carry. `exp`, `aud` and `iss` are checked by the
/// validator; `scope` is required to be present but not interpreted.
#[derive(Clone, Debug, Deserialize)]
struct Claims {
    sub: String,
    #[allow(dead_code)]
    scope: String,
}

impl BearerAuth {
    /// `public_key_pem_base64` is an EC public key in PEM form, encoded as URL-safe base64.
    pub fn new(aud: &str, iss: &str, public_key_pem_base64: &str) -> anyhow::Result<Self> {
        let mut validation = Validation::new(ES256);
        validation.validate_exp = true;
        validation.validate_aud = true;
        validation.set_audience(&[aud]);
        validation.set_issuer(&[iss]);

        let pem = base64::engine::general_purpose::URL_SAFE
            .decode(public_key_pem_base64)
            .context("Failed to decode the public key from base64")?;
        let decoding_key =
            DecodingKey::from_ec_pem(&pem).context("Failed to parse the EC public key")?;

        Ok(Self {
            inner: Arc::new(Inner {
                validation,
                decoding_key,
            }),
        })
    }

    /// Validates `token` and returns the user it names. The role is never set from the token.
    pub fn authenticate(&self, token: &str) -> Result<AuthenticatedUser, ApplicationError> {
        let data =
            jsonwebtoken::decode::<Claims>(token, &self.inner.decoding_key, &self.inner.validation)
                .map_err(|_| ApplicationError::Unauthorized)?;

        Ok(AuthenticatedUser {
            id: data.claims.sub,
            role: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{EncodingKey, Header};
    use serde::Serialize;

    // A throwaway P-256 key pair that exists only for these tests.
    const PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgIOokwRIWAZ1ZfEIV
L1slXGvZBohmiTmuU4TCjrl+xtmhRANCAAR1jQtuUFY7wK2S1KTV5ULO35jAXXRF
PCueAM+8TjOLoQ90xYm8Zhz6l3z+CKPaupm7hVv4TTg+H97j5Br2MzEZ
-----END PRIVATE KEY-----
";
    const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEdY0LblBWO8CtktSk1eVCzt+YwF10
RTwrngDPvE4zi6EPdMWJvGYc+pd8/gij2rqZu4Vb+E04Ph/e4+Qa9jMxGQ==
-----END PUBLIC KEY-----
";

    #[derive(Serialize)]
    struct TestClaims<'a> {
        sub: &'a str,
        aud: &'a str,
        iss: &'a str,
        scope: &'a str,
        exp: u64,
    }

    fn token(aud: &str, exp: u64) -> String {
        let claims = TestClaims {
            sub: "user-1",
            aud,
            iss: "issuer",
            scope: "read",
            exp,
        };
        let key = EncodingKey::from_ec_pem(PRIVATE_KEY.as_bytes()).unwrap();
        jsonwebtoken::encode(&Header::new(ES256), &claims, &key).unwrap()
    }

    fn auth() -> BearerAuth {
        let key = base64::engine::general_purpose::URL_SAFE.encode(PUBLIC_KEY);
        BearerAuth::new("api", "issuer", &key).unwrap()
    }

    const FAR_FUTURE: u64 = 4_102_444_800;

    #[test]
    fn accepts_a_valid_token() {
        let user = auth().authenticate(&token("api", FAR_FUTURE)).unwrap();
        assert_eq!(user.id, "user-1");
        assert_eq!(user.role, None);
    }

    #[test]
    fn rejects_the_wrong_audience() {
        assert!(matches!(
            auth().authenticate(&token("other", FAR_FUTURE)),
            Err(ApplicationError::Unauthorized)
        ));
    }

    #[test]
    fn rejects_an_expired_token() {
        assert!(matches!(
            auth().authenticate(&token("api", 1)),
            Err(ApplicationError::Unauthorized)
        ));
    }

    #[test]
    fn rejects_garbage() {
        assert!(matches!(
            auth().authenticate("not-a-jwt"),
            Err(ApplicationError::Unauthorized)
        ));
    }
}
