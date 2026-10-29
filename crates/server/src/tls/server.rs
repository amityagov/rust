use std::sync::Arc;

use anyhow::{Context, bail};
use rustls::{ServerConfig, pki_types::PrivateKeyDer, server::WebPkiClientVerifier};
use rustls_pki_types::pem::PemObject;

use super::Source;

pub struct TlsServerConfig {
    inner: ServerConfig,
}

impl TlsServerConfig {
    pub(crate) fn from_sources(
        server_cert: Source,
        server_key: Source,
        root_ca: Source,
    ) -> anyhow::Result<Self> {
        let root_store = super::ca::load_root_ca_from_value(&root_ca.resolve("root_ca")?)?;
        let verifier = WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .context("Failed to build client certificate verifier")?;

        let server_cert = super::ca::parse_certificates(&server_cert.resolve("server_cert")?)?;
        let server_key = load_key(&server_key.resolve("server_key")?)?;

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

fn load_key(raw: &str) -> anyhow::Result<PrivateKeyDer<'static>> {
    let pem_bytes = super::decode_base64_pem(raw)?;

    match PrivateKeyDer::from_pem_slice(&pem_bytes) {
        Ok(key) => Ok(key),
        Err(_) => bail!("Failed to parse private key; expected PKCS#8, PKCS#1 or SEC1 PEM"),
    }
}
