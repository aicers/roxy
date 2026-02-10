//! Configuration and settings for `roxyd`.
//!
//! This module consolidates CLI argument parsing and TOML configuration loading.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Parser;
use config::Config as RawConfig;
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

/// File-backed configuration for `roxyd`.
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    /// Path to the log file.
    pub log_path: Option<PathBuf>,
}

impl Config {
    /// Loads configuration from the specified path.
    ///
    /// # Errors
    ///
    /// Returns an error if the config file cannot be read or parsed.
    pub fn load(path: &Path) -> Result<Self> {
        let settings = RawConfig::builder()
            .add_source(config::File::from(path))
            .add_source(config::Environment::with_prefix("ROXYD").try_parsing(true))
            .build()
            .with_context(|| format!("Failed to load config from: {}", path.display()))?;

        let config: Self = settings
            .try_deserialize()
            .with_context(|| format!("Failed to parse config: {}", path.display()))?;

        Ok(config)
    }
}

/// Runtime settings for the `roxyd` daemon.
#[derive(Debug, Clone)]
pub struct Settings {
    pub manager_server: String,
    pub cert: PathBuf,
    pub key: PathBuf,
    pub ca_certs: Vec<PathBuf>,
    pub config: Config,
}

impl Settings {
    pub fn from_args(args: &Args, config: Config) -> Self {
        Self {
            manager_server: args.manager_server.clone(),
            cert: args.cert.clone(),
            key: args.key.clone(),
            ca_certs: args.ca_certs.clone(),
            config,
        }
    }

    pub fn log_path(&self) -> Option<&std::path::Path> {
        self.config.log_path.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        ffi::OsString,
        io::Write,
        path::PathBuf,
        sync::{Mutex, OnceLock},
    };

    use tempfile::{Builder, NamedTempFile, tempdir};

    use super::Config;

    const LOG_PATH_ENV_KEY: &str = "ROXYD_LOG_PATH";

    fn env_lock() -> &'static Mutex<()> {
        static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_env() -> std::sync::MutexGuard<'static, ()> {
        env_lock()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    struct EnvVarGuard {
        key: &'static str,
        original_value: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &str) -> Self {
            let original_value = std::env::var_os(key);
            // Safety: tests mutate process environment under a global lock.
            unsafe { std::env::set_var(key, value) };
            Self {
                key,
                original_value,
            }
        }

        fn unset(key: &'static str) -> Self {
            let original_value = std::env::var_os(key);
            // Safety: tests mutate process environment under a global lock.
            unsafe { std::env::remove_var(key) };
            Self {
                key,
                original_value,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.original_value.take() {
                Some(value) => {
                    // Safety: restoration runs while holding the same global lock.
                    unsafe { std::env::set_var(self.key, value) };
                }
                None => {
                    // Safety: restoration runs while holding the same global lock.
                    unsafe { std::env::remove_var(self.key) };
                }
            }
        }
    }

    fn write_temp_config(contents: &str) -> NamedTempFile {
        let mut file = Builder::new()
            .suffix(".toml")
            .tempfile()
            .expect("Failed to create temp config file");
        write!(file, "{contents}").expect("Failed to write config file");
        file
    }

    #[test]
    fn load_reads_log_path_from_toml() {
        let _guard = lock_env();
        let _env_guard = EnvVarGuard::unset(LOG_PATH_ENV_KEY);
        let file = write_temp_config("log_path = \"/tmp/roxyd-toml.log\"\n");

        let config = Config::load(file.path()).expect("Failed to load config");
        assert_eq!(config.log_path, Some(PathBuf::from("/tmp/roxyd-toml.log")));
    }

    #[test]
    fn load_uses_env_to_override_toml() {
        let _guard = lock_env();
        let _env_guard = EnvVarGuard::set(LOG_PATH_ENV_KEY, "/tmp/roxyd-env.log");
        let file = write_temp_config("log_path = \"/tmp/roxyd-toml.log\"\n");

        let config = Config::load(file.path()).expect("Failed to load config");
        assert_eq!(config.log_path, Some(PathBuf::from("/tmp/roxyd-env.log")));
    }

    #[test]
    fn load_fails_when_config_file_is_missing() {
        let _guard = lock_env();
        let _env_guard = EnvVarGuard::unset(LOG_PATH_ENV_KEY);
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let missing_path = temp_dir.path().join("missing.toml");

        let err = Config::load(&missing_path).expect_err("Expected load to fail for missing file");
        assert!(
            err.to_string().contains("Failed to load config from"),
            "Unexpected error message: {err}"
        );
    }

    #[test]
    fn load_fails_when_config_value_type_is_invalid() {
        let _guard = lock_env();
        let _env_guard = EnvVarGuard::unset(LOG_PATH_ENV_KEY);
        let file = write_temp_config("log_path = { nested = \"value\" }\n");

        let err =
            Config::load(file.path()).expect_err("Expected load to fail for invalid value type");
        assert!(
            err.to_string().contains("Failed to parse config"),
            "Unexpected error message: {err}"
        );
    }
}
