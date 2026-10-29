use anyhow::{Context, bail};
use rustls::RootCertStore;
use rustls_pki_types::{CertificateDer, pem::PemObject};

/// Loads the root CA store from the `ROOT_CA` environment variable.
pub fn load_root_ca() -> anyhow::Result<RootCertStore> {
    load_root_ca_from_env("ROOT_CA")
}

pub fn load_root_ca_from_env(env: &str) -> anyhow::Result<RootCertStore> {
    let raw =
        std::env::var(env).with_context(|| format!("`{env}` environment variable is not set"))?;
    load_root_ca_from_value(&raw)
}

pub fn load_root_ca_from_value(raw: &str) -> anyhow::Result<RootCertStore> {
    let certs = parse_certificates(raw)?;

    let mut root_store = RootCertStore::empty();
    root_store.add_parsable_certificates(certs);
    Ok(root_store)
}

/// Parses every certificate of a base64-encoded PEM bundle; an empty bundle is an error.
pub(super) fn parse_certificates(raw: &str) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let pem_bytes = super::decode_base64_pem(raw)?;

    let certs = CertificateDer::pem_slice_iter(&pem_bytes)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse PEM certificates")?;

    if certs.is_empty() {
        bail!("No certificates found");
    }

    Ok(certs)
}
