use anyhow::{Context, bail};
use rustls::RootCertStore;
use rustls_pki_types::{CertificateDer, pem::PemObject};

pub fn load_root_ca() -> anyhow::Result<RootCertStore> {
    load_root_ca_from_env("ROOT_CA")
}

pub fn load_root_ca_from_env(env: &str) -> anyhow::Result<RootCertStore> {
    let raw = std::env::var(env).context(format!("`{env}` environment variable is not set"))?;
    load_root_ca_from_value(&raw)
}

pub fn load_root_ca_from_value(raw: &str) -> anyhow::Result<RootCertStore> {
    let pem_bytes = super::decode_base64_pem(raw)?;

    let certs = CertificateDer::pem_slice_iter(&pem_bytes)
        .collect::<Result<Vec<_>, _>>()
        .context("Failed to parse PEM certificates")?;

    if certs.is_empty() {
        bail!("No certificates found");
    }

    let mut root_store = RootCertStore::empty();
    root_store.add_parsable_certificates(certs);
    Ok(root_store)
}
