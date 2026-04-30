use std::sync::Arc;

use anyhow::{Context, bail};
use rustls::{
    ServerConfig,
    pki_types::{CertificateDer, PrivateKeyDer},
    server::WebPkiClientVerifier,
};
use rustls_pki_types::pem::PemObject;

use super::{CaSource, CertSource, KeySource};

pub struct TlsServerConfig {
    inner: ServerConfig,
}

impl TlsServerConfig {
    pub(crate) fn from_sources(
        server_cert: CertSource,
        server_key: KeySource,
        root_ca: CaSource,
    ) -> anyhow::Result<Self> {
        let root_store = match root_ca {
            CaSource::Env(env) => super::ca::load_root_ca_from_env(env)?,
            CaSource::Value(base64) => super::ca::load_root_ca_from_value(&base64)?,
            CaSource::None => bail!("root_ca is required"),
        };

        let verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .context("Failed to build client certificate verifier")?;

        let server_cert = match server_cert {
            CertSource::Env(env) => load_cert_from_env(env)?,
            CertSource::Value(base64) => load_cert_from_value(&base64)?,
            CertSource::None => bail!("server_cert is required"),
        };

        let server_key = match server_key {
            KeySource::Env(env) => load_key_from_env(env)?,
            KeySource::Value(base64) => load_key_from_value(&base64)?,
            KeySource::None => bail!("server_key is required"),
        };

        let config = ServerConfig::builder()
            .with_client_cert_verifier(verifier)
            .with_single_cert(server_cert, server_key)
            .context("Failed to build rustls ServerConfig")?;

        Ok(Self { inner: config })
    }

    pub fn into_inner(self) -> ServerConfig {
        self.inner
    }
}

fn load_cert_from_env(env: &str) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let raw = std::env::var(env).context(format!("`{env}` environment variable is not set"))?;
    load_cert_from_value(&raw)
}

fn load_cert_from_value(raw: &str) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let pem_bytes = super::decode_base64_pem(raw)?;

    let certs = CertificateDer::pem_slice_iter(&pem_bytes)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse PEM certificates")?;

    if certs.is_empty() {
        bail!("No certificates found");
    }

    Ok(certs)
}

fn load_key_from_env(env: &str) -> anyhow::Result<PrivateKeyDer<'static>> {
    let raw = std::env::var(env).context(format!("`{env}` environment variable is not set"))?;
    load_key_from_value(&raw)
}

fn load_key_from_value(raw: &str) -> anyhow::Result<PrivateKeyDer<'static>> {
    let pem_bytes = super::decode_base64_pem(raw)?;

    if let Ok(key) = PrivateKeyDer::from_pem_slice(&pem_bytes) {
        return Ok(key);
    }

    bail!("Failed to parse private key; expected PKCS#8 or PKCS#1 PEM")
}
