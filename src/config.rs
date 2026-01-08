//! Configuration structures for `roxyd`.
//!
//! This module defines configuration types for the new `roxyd` binary,
//! specifically for QUIC and mTLS connectivity with the Manager.
//!
//! All fields are optional to ensure legacy configurations remain unaffected.
//! If no `roxyd` configuration is present, the `roxyd` binary will use defaults
//! or exit gracefully with an informative message.

use serde::{Deserialize, Serialize};

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
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct RoxydConfig {
    /// Address of the Manager to connect to (e.g., "192.168.1.100:4433").
    ///
    /// This is the QUIC endpoint where the Manager listens for connections.
    pub manager_address: Option<String>,

    /// QUIC transport configuration.
    pub quic: Option<QuicConfig>,

    /// mTLS certificate configuration for secure authentication.
    pub mtls: Option<MtlsConfig>,
}

/// QUIC transport configuration.
///
/// These settings control the QUIC connection behavior when connecting to
/// the Manager.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct QuicConfig {
    /// Local address to bind for outgoing QUIC connections.
    ///
    /// If not specified, the system will choose an available address and port.
    /// Example: "0.0.0.0:0" for any available port.
    pub bind_address: Option<String>,

    /// Connection idle timeout in milliseconds.
    ///
    /// The connection will be closed if no data is exchanged within this period.
    /// If not specified, a reasonable default will be used by the QUIC implementation.
    pub idle_timeout_ms: Option<u64>,
}

/// mTLS (mutual TLS) certificate configuration.
///
/// These paths point to the certificate files needed for mTLS authentication
/// with the Manager. Both the client certificate and CA certificate are required
/// for mutual authentication.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct MtlsConfig {
    /// Path to the client certificate file (PEM format).
    ///
    /// This certificate identifies `roxyd` to the Manager.
    pub cert_path: Option<String>,

    /// Path to the private key file (PEM format).
    ///
    /// The private key corresponding to the client certificate.
    pub key_path: Option<String>,

    /// Path to the CA certificate file (PEM format).
    ///
    /// The CA certificate used to verify the Manager's certificate.
    pub ca_cert_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RoxydConfig::default();
        assert!(config.manager_address.is_none());
        assert!(config.quic.is_none());
        assert!(config.mtls.is_none());
    }

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

        assert_eq!(
            config.manager_address,
            Some("192.168.1.100:4433".to_string())
        );

        let quic = config.quic.expect("QUIC config should be present");
        assert_eq!(quic.bind_address, Some("0.0.0.0:0".to_string()));
        assert_eq!(quic.idle_timeout_ms, Some(30000));

        let mtls = config.mtls.expect("mTLS config should be present");
        assert_eq!(mtls.cert_path, Some("/etc/roxyd/cert.pem".to_string()));
        assert_eq!(mtls.key_path, Some("/etc/roxyd/key.pem".to_string()));
        assert_eq!(mtls.ca_cert_path, Some("/etc/roxyd/ca.pem".to_string()));
    }

    #[test]
    fn test_deserialize_partial_config() {
        let toml_str = r#"
            manager_address = "192.168.1.100:4433"
        "#;

        let config: RoxydConfig = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert_eq!(
            config.manager_address,
            Some("192.168.1.100:4433".to_string())
        );
        assert!(config.quic.is_none());
        assert!(config.mtls.is_none());
    }

    #[test]
    fn test_deserialize_empty_config() {
        let toml_str = "";

        let config: RoxydConfig = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert!(config.manager_address.is_none());
        assert!(config.quic.is_none());
        assert!(config.mtls.is_none());
    }

    #[test]
    fn test_serialize_config() {
        let config = RoxydConfig {
            manager_address: Some("192.168.1.100:4433".to_string()),
            quic: Some(QuicConfig {
                bind_address: Some("0.0.0.0:0".to_string()),
                idle_timeout_ms: Some(30000),
            }),
            mtls: Some(MtlsConfig {
                cert_path: Some("/etc/roxyd/cert.pem".to_string()),
                key_path: Some("/etc/roxyd/key.pem".to_string()),
                ca_cert_path: Some("/etc/roxyd/ca.pem".to_string()),
            }),
        };

        let toml_str = toml::to_string(&config).expect("Failed to serialize to TOML");
        assert!(toml_str.contains("manager_address"));
        assert!(toml_str.contains("192.168.1.100:4433"));
    }
}
