pub mod ca;
pub mod server;

use std::sync::Arc;

use anyhow::Context;
pub use axum_server::tls_rustls::RustlsConfig;
use base64::Engine;

use self::server::TlsServerConfig;

#[derive(Default)]
pub struct TlsBuilder {
    server_cert: CertSource,
    server_key: KeySource,
    root_ca: CaSource,
}

pub(crate) enum CertSource {
    Env(&'static str),
    Value(String),
    None,
}

impl Default for CertSource {
    fn default() -> Self {
        Self::None
    }
}

pub(crate) enum KeySource {
    Env(&'static str),
    Value(String),
    None,
}

impl Default for KeySource {
    fn default() -> Self {
        Self::None
    }
}

pub(crate) enum CaSource {
    Env(&'static str),
    Value(String),
    None,
}

impl Default for CaSource {
    fn default() -> Self {
        Self::None
    }
}

impl TlsBuilder {
    pub fn server_cert_from_env(mut self, env: &'static str) -> Self {
        self.server_cert = CertSource::Env(env);
        self
    }

    pub fn server_cert(mut self, base64_encoded: impl Into<String>) -> Self {
        self.server_cert = CertSource::Value(base64_encoded.into());
        self
    }

    pub fn server_key_from_env(mut self, env: &'static str) -> Self {
        self.server_key = KeySource::Env(env);
        self
    }

    pub fn server_key(mut self, base64_encoded: impl Into<String>) -> Self {
        self.server_key = KeySource::Value(base64_encoded.into());
        self
    }

    pub fn root_ca_from_env(mut self, env: &'static str) -> Self {
        self.root_ca = CaSource::Env(env);
        self
    }

    pub fn root_ca(mut self, base64_encoded: impl Into<String>) -> Self {
        self.root_ca = CaSource::Value(base64_encoded.into());
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

pub(crate) fn decode_base64_pem(raw: &str) -> anyhow::Result<Vec<u8>> {
    let normalized = raw
        .bytes()
        .filter(|byte| !byte.is_ascii_whitespace())
        .collect::<Vec<_>>();

    base64::engine::general_purpose::STANDARD
        .decode(normalized)
        .context("Failed to decode base64")
}
