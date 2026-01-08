//! `roxyd` - New implementation path for QUIC/mTLS connectivity with Manager.
//!
//! This is a skeleton binary entrypoint that coexists with the legacy `roxy` binary.
//! It provides configuration loading, tracing initialization, and async runtime
//! bootstrap, but does not yet implement any review-protocol request handling.
//!
//! # Usage
//!
//! ```sh
//! cargo run --bin roxyd -- --config path/to/config.toml
//! ```

use std::{env, fs, path::PathBuf, process};

use anyhow::{Context, Result};
use roxy::config::{MtlsConfig, QuicConfig, RoxydConfig};
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_LOG_PATH: &str = "/opt/clumit/log/roxyd.log";

/// Initializes tracing/logging infrastructure.
///
/// Uses a file appender with non-blocking writes for performance. The log level
/// can be controlled via the `RUST_LOG` environment variable, defaulting to INFO.
///
/// # Errors
///
/// Returns an error if the log file cannot be opened or created.
fn init_tracing() -> Result<WorkerGuard> {
    let log_path = DEFAULT_LOG_PATH;
    let (layer, guard) = {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .with_context(|| format!("Failed to open the log file: {log_path}"))?;
        let (non_blocking, file_guard) = tracing_appender::non_blocking(file);
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy();
        (
            fmt::Layer::default()
                .with_ansi(false)
                .with_target(false)
                .with_writer(non_blocking)
                .with_filter(env_filter),
            file_guard,
        )
    };

    tracing_subscriber::Registry::default().with(layer).init();
    Ok(guard)
}

/// Parses command-line arguments to extract the config file path.
///
/// Supports `--config <PATH>` or `-c <PATH>` syntax.
fn parse_args() -> Option<PathBuf> {
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if (args[i] == "--config" || args[i] == "-c") && i + 1 < args.len() {
            return Some(PathBuf::from(&args[i + 1]));
        }
        i += 1;
    }
    None
}

/// Loads configuration from the specified path.
///
/// # Errors
///
/// Returns an error if:
/// * The config file cannot be read
/// * The config file contains invalid TOML syntax
fn load_config(path: &PathBuf) -> Result<RoxydConfig> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config: {}", path.display()))?;
    toml::from_str(&contents).with_context(|| format!("Failed to parse config: {}", path.display()))
}

/// Logs the current configuration state for debugging purposes.
fn log_config_status(config: &RoxydConfig) {
    tracing::info!("roxyd started");

    if let Some(addr) = &config.manager_address {
        tracing::info!("Manager address: {addr}");
    } else {
        tracing::info!("Manager address: not configured");
    }

    if let Some(quic) = &config.quic {
        log_quic_config(quic);
    } else {
        tracing::info!("QUIC config: not configured");
    }

    if let Some(mtls) = &config.mtls {
        log_mtls_config(mtls);
    } else {
        tracing::info!("mTLS config: not configured");
    }
}

/// Logs QUIC configuration details.
fn log_quic_config(quic: &QuicConfig) {
    tracing::info!(
        "QUIC config: bind_address={:?}, idle_timeout_ms={:?}",
        quic.bind_address,
        quic.idle_timeout_ms
    );
}

/// Logs mTLS configuration details.
fn log_mtls_config(mtls: &MtlsConfig) {
    tracing::info!(
        "mTLS config: cert_path={:?}, key_path={:?}, ca_cert_path={:?}",
        mtls.cert_path,
        mtls.key_path,
        mtls.ca_cert_path
    );
}

#[tokio::main]
async fn main() {
    // Initialize tracing first so we can log any subsequent errors
    let _guard = match init_tracing() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize tracing: {e}");
            process::exit(1);
        }
    };

    // Parse CLI arguments for config path
    let config_path = parse_args();

    // Load configuration if path provided, otherwise use defaults
    let config = if let Some(ref path) = config_path {
        match load_config(path) {
            Ok(cfg) => {
                tracing::info!("Loaded config from: {path:?}");
                cfg
            }
            Err(e) => {
                tracing::error!("Failed to load config: {e}");
                eprintln!("Failed to load config from {}: {e}", path.display());
                process::exit(1);
            }
        }
    } else {
        tracing::info!("No config file specified, using defaults");
        RoxydConfig::default()
    };

    // Log configuration status
    log_config_status(&config);

    tracing::info!("roxyd is running (skeleton mode - no protocol handlers active)");

    // Await indefinitely - this is a skeleton that doesn't implement any handlers yet
    // In the future, this will be replaced with actual QUIC/mTLS connection handling
    std::future::pending::<()>().await;
}
