//! Configuration and settings for `roxyd`.
//!
//! This module consolidates CLI argument parsing and TOML configuration loading.

use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

/// roxyd - QUIC/mTLS connectivity daemon for Manager communication
#[derive(Parser, Debug)]
#[command(name = "roxyd")]
#[command(about = "QUIC/mTLS connectivity daemon for Manager communication")]
pub struct Args {
    /// Path to the configuration file (TOML format)
    #[arg(short = 'c', value_name = "CONFIG_PATH")]
    pub config: PathBuf,

    /// Path to the certificate file.
    #[arg(long, value_name = "CERT_PATH")]
    pub cert: PathBuf,

    /// Path to the key file.
    #[arg(long, value_name = "KEY_PATH")]
    pub key: PathBuf,

    /// Paths to the CA certificate files. Multiple paths can be provided as a comma-separated list.
    #[arg(
        long,
        value_name = "CA_CERTS_PATHS",
        required = true,
        value_delimiter = ','
    )]
    pub ca_certs: Vec<PathBuf>,

    /// Address of the Manager server formatted as `<server_name>@<server_ip>:<server_port>`.
    #[arg(value_name = "MANAGER_SERVER")]
    pub manager_server: String,
}

/// File-backed configuration for the `roxyd` daemon.
#[derive(Debug, Clone, Deserialize)]
pub struct RoxydFileConfig {
    /// Path to the log file.
    pub log_path: Option<PathBuf>,
}

/// Runtime configuration for the `roxyd` daemon.
#[derive(Debug, Clone)]
pub struct RoxydConfig {
    pub manager_server: String,
    pub cert: PathBuf,
    pub key: PathBuf,
    pub ca_certs: Vec<PathBuf>,
    pub log_path: Option<PathBuf>,
}

impl RoxydConfig {
    pub fn from_args(args: &Args, file: RoxydFileConfig) -> Self {
        Self {
            manager_server: args.manager_server.clone(),
            cert: args.cert.clone(),
            key: args.key.clone(),
            ca_certs: args.ca_certs.clone(),
            log_path: file.log_path,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_full_config() {
        let toml_str = r#"
            log_path = "/opt/clumit/log/roxyd.log"
        "#;

        let config: RoxydFileConfig = toml::from_str(toml_str).expect("Failed to parse TOML");

        assert_eq!(
            config.log_path,
            Some(PathBuf::from("/opt/clumit/log/roxyd.log"))
        );
    }

    #[test]
    fn test_deserialize_empty_config() {
        let config: RoxydFileConfig = toml::from_str("").expect("Failed to parse TOML");
        assert!(config.log_path.is_none());
    }
}
