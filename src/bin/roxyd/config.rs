//! Configuration structures for `roxyd`.

use std::path::PathBuf;

use serde::Deserialize;

/// Configuration for the `roxyd` daemon.
#[derive(Debug, Clone, Deserialize)]
pub struct RoxydConfig {
    /// Address of the Manager to connect to (e.g., "192.168.1.100:4433").
    pub manager_address: String,

    pub quic: QuicConfig,

    pub mtls: MtlsConfig,
}

/// QUIC transport configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct QuicConfig {
    /// Local address to bind (e.g., "0.0.0.0:0").
    pub bind_address: String,

    /// Connection idle timeout in milliseconds.
    pub idle_timeout_ms: u64,
}

/// mTLS certificate configuration.
#[derive(Debug, Clone, Deserialize)]
#[allow(clippy::struct_field_names)]
pub struct MtlsConfig {
    /// Path to the client certificate (PEM).
    pub cert_path: PathBuf,

    /// Path to the private key (PEM).
    pub key_path: PathBuf,

    /// Path to the CA certificate (PEM).
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
