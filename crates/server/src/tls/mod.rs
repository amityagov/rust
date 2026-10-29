//! Mutual-TLS server configuration assembled from base64-encoded PEM material.

pub mod ca;
pub mod server;

use std::sync::Arc;

use anyhow::Context;
pub use axum_server::tls_rustls::RustlsConfig;
use base64::Engine;

use self::server::TlsServerConfig;

/// Where a piece of PEM material comes from.
#[derive(Default)]
pub(crate) enum Source {
    /// The name of an environment variable holding the base64-encoded PEM.
    Env(&'static str),
    /// The base64-encoded PEM itself.
    Value(String),
    /// Not provided yet.
    #[default]
    None,
}

impl Source {
    /// The base64 text, or an error naming `what` if the source is empty or unset.
    pub(crate) fn resolve(&self, what: &str) -> anyhow::Result<String> {
        match self {
            Source::Env(name) => std::env::var(name)
                .with_context(|| format!("`{name}` environment variable is not set")),
            Source::Value(value) => Ok(value.clone()),
            Source::None => anyhow::bail!("{what} is required"),
        }
    }
}

/// Builds the [`RustlsConfig`] of a server that requires client certificates.
///
/// The server certificate (chain), its private key and the CA that signs client certificates are
/// all required; each can be given directly or read from an environment variable.
#[derive(Default)]
pub struct TlsBuilder {
    server_cert: Source,
    server_key: Source,
    root_ca: Source,
}

impl TlsBuilder {
    pub fn server_cert_from_env(mut self, env: &'static str) -> Self {
        self.server_cert = Source::Env(env);
        self
    }

    pub fn server_cert(mut self, base64_encoded: impl Into<String>) -> Self {
        self.server_cert = Source::Value(base64_encoded.into());
        self
    }

    pub fn server_key_from_env(mut self, env: &'static str) -> Self {
        self.server_key = Source::Env(env);
        self
    }

    pub fn server_key(mut self, base64_encoded: impl Into<String>) -> Self {
        self.server_key = Source::Value(base64_encoded.into());
        self
    }

    pub fn root_ca_from_env(mut self, env: &'static str) -> Self {
        self.root_ca = Source::Env(env);
        self
    }

    pub fn root_ca(mut self, base64_encoded: impl Into<String>) -> Self {
        self.root_ca = Source::Value(base64_encoded.into());
        self
    }

    pub fn build(self) -> anyhow::Result<RustlsConfig> {
        let server_config =
            TlsServerConfig::from_sources(self.server_cert, self.server_key, self.root_ca)?;
        Ok(RustlsConfig::from_config(Arc::new(
            server_config.into_inner(),
        )))
    }
}

pub fn tls() -> TlsBuilder {
    TlsBuilder::default()
}

/// Decodes base64 text, ignoring any whitespace (PEM is often pasted with line breaks).
pub(crate) fn decode_base64_pem(raw: &str) -> anyhow::Result<Vec<u8>> {
    let compact: Vec<u8> = raw
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect();

    base64::engine::general_purpose::STANDARD
        .decode(compact)
        .context("Failed to decode base64")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_may_contain_whitespace() {
        assert_eq!(decode_base64_pem("aGVs\n bG8=\r\n").unwrap(), b"hello");
    }

    #[test]
    fn an_unset_source_names_what_is_missing() {
        let error = Source::None.resolve("server_cert").unwrap_err();
        assert_eq!(error.to_string(), "server_cert is required");
    }

    #[test]
    fn a_missing_variable_is_reported_by_name() {
        let error = Source::Env("TLS_TEST_VARIABLE_THAT_IS_NOT_SET")
            .resolve("root_ca")
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("TLS_TEST_VARIABLE_THAT_IS_NOT_SET")
        );
    }

    #[test]
    fn building_without_material_fails() {
        assert!(tls().build().is_err());
    }
}
