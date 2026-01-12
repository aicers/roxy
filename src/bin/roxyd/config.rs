//! Configuration structures for `roxyd`.
//!
//! This module defines configuration types for the `roxyd` binary,
//! specifically for QUIC and mTLS connectivity with the Manager.

use std::path::PathBuf;

use serde::Deserialize;

/// Configuration for the `roxyd` daemon.
///
/// This structure holds all settings needed for `roxyd` to connect to the Manager
/// via QUIC with mTLS authentication.
///
/// # Example
///
/// ```toml
/// manager_address = "192.168.1.100:4433"
///
/// [quic]
/// bind_address = "0.0.0.0:0"
/// idle_timeout_ms = 30000
///
/// [mtls]
/// cert_path = "/etc/roxyd/cert.pem"
/// key_path = "/etc/roxyd/key.pem"
/// ca_cert_path = "/etc/roxyd/ca.pem"
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct RoxydConfig {
    /// Address of the Manager to connect to (e.g., "192.168.1.100:4433").
    ///
    /// This is the QUIC endpoint where the Manager listens for connections.
    pub manager_address: String,

    /// QUIC transport configuration.
    pub quic: QuicConfig,

    /// mTLS certificate configuration for secure authentication.
    pub mtls: MtlsConfig,
}

/// QUIC transport configuration.
///
/// These settings control the QUIC connection behavior when connecting to
/// the Manager.
#[derive(Debug, Clone, Deserialize)]
pub struct QuicConfig {
    /// Local address to bind for outgoing QUIC connections.
    ///
    /// Example: "0.0.0.0:0" for any available port.
    pub bind_address: String,

    /// Connection idle timeout in milliseconds.
    ///
    /// The connection will be closed if no data is exchanged within this period.
    pub idle_timeout_ms: u64,
}

/// mTLS (mutual TLS) certificate configuration.
///
/// These paths point to the certificate files needed for mTLS authentication
/// with the Manager. Both the client certificate and CA certificate are required
/// for mutual authentication.
#[derive(Debug, Clone, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct MtlsConfig {
    /// Path to the client certificate file (PEM format).
    ///
    /// This certificate identifies `roxyd` to the Manager.
    pub cert_path: PathBuf,

    /// Path to the private key file (PEM format).
    ///
    /// The private key corresponding to the client certificate.
    pub key_path: PathBuf,

    /// Path to the CA certificate file (PEM format).
    ///
    /// The CA certificate used to verify the Manager's certificate.
    pub ca_cert_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_config() {
        let toml_str = r#"
            manager_address = "192.168.1.100:4433"

            [quic]
            bind_address = "0.0.0.0:0"
            idle_timeout_ms = 30000

            [mtls]
            cert_path = "/etc/roxyd/cert.pem"
            key_path = "/etc/roxyd/key.pem"
            ca_cert_path = "/etc/roxyd/ca.pem"
        "#;

        let config: RoxydConfig = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert_eq!(config.manager_address, "192.168.1.100:4433");
        assert_eq!(config.quic.bind_address, "0.0.0.0:0");
        assert_eq!(config.quic.idle_timeout_ms, 30000);
        assert_eq!(config.mtls.cert_path, PathBuf::from("/etc/roxyd/cert.pem"));
        assert_eq!(config.mtls.key_path, PathBuf::from("/etc/roxyd/key.pem"));
        assert_eq!(config.mtls.ca_cert_path, PathBuf::from("/etc/roxyd/ca.pem"));
    }

    #[test]
    fn test_missing_required_field_fails() {
        let toml_str = r#"
            [quic]
            bind_address = "0.0.0.0:0"
            idle_timeout_ms = 30000
        "#;

        let result: Result<RoxydConfig, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }
}
