//! TLS / mutual-TLS termination for the internal Ledgerscope.Accounts (C#)
//! ↔ Rust boundary. Enabled via the `server.tls` config block; when
//! `require_client_auth` is set, only callers presenting a certificate
//! signed by `client_ca_path` may connect (mTLS).

use std::fs;
use std::io::BufReader;
use std::sync::Arc;

use anyhow::Context;
use axum_server::tls_rustls::RustlsConfig;
use rustls::server::WebPkiClientVerifier;
use rustls::{RootCertStore, ServerConfig};
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

use crate::config::TlsConfig;

/// Build an axum-server `RustlsConfig` from the `server.tls` block. Uses the
/// `ring` crypto provider explicitly so nothing depends on a process-global
/// default being installed elsewhere.
pub fn build_rustls_config(cfg: &TlsConfig) -> anyhow::Result<RustlsConfig> {
    let provider = Arc::new(rustls::crypto::ring::default_provider());

    let certs = load_certs(cfg.cert_path())?;
    let key = load_key(cfg.key_path())?;

    let builder = ServerConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .context("configuring TLS protocol versions")?;

    let server_config = if cfg.require_client_auth() {
        let ca_path = cfg.client_ca_path().context(
            "server.tls.require_client_auth = true requires server.tls.client_ca_path",
        )?;
        let mut roots = RootCertStore::empty();
        for cert in load_certs(ca_path)? {
            roots
                .add(cert)
                .with_context(|| format!("adding client CA cert from {ca_path}"))?;
        }
        let verifier = WebPkiClientVerifier::builder_with_provider(Arc::new(roots), provider)
            .build()
            .context("building client certificate verifier (mTLS)")?;
        builder
            .with_client_cert_verifier(verifier)
            .with_single_cert(certs, key)
            .context("loading server certificate/key")?
    } else {
        builder
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .context("loading server certificate/key")?
    };

    Ok(RustlsConfig::from_config(Arc::new(server_config)))
}

fn load_certs(path: &str) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let data = fs::read(path).with_context(|| format!("reading cert file {path}"))?;
    let certs = rustls_pemfile::certs(&mut BufReader::new(&data[..]))
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("parsing PEM certificates from {path}"))?;
    anyhow::ensure!(!certs.is_empty(), "no certificates found in {path}");
    Ok(certs)
}

fn load_key(path: &str) -> anyhow::Result<PrivateKeyDer<'static>> {
    let data = fs::read(path).with_context(|| format!("reading key file {path}"))?;
    rustls_pemfile::private_key(&mut BufReader::new(&data[..]))
        .with_context(|| format!("parsing private key from {path}"))?
        .with_context(|| format!("no private key found in {path}"))
}
