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

mod config;

use std::{fs, path::Path, path::PathBuf, process};

use anyhow::{Context, Result};
use clap::Parser;
use config::{MtlsConfig, QuicConfig, RoxydConfig};
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

const DEFAULT_LOG_PATH: &str = "/opt/clumit/log/roxyd.log";

/// roxyd - QUIC/mTLS connectivity daemon for Manager communication
#[derive(Parser, Debug)]
#[command(name = "roxyd")]
#[command(about = "QUIC/mTLS connectivity daemon for Manager communication")]
struct Args {
    /// Path to the configuration file (TOML format)
    #[arg(short, long)]
    config: PathBuf,
}

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

/// Loads configuration from the specified path with environment variable overrides.
///
/// # Errors
///
/// Returns an error if the config file cannot be read or parsed.
fn load_config(path: &Path) -> Result<RoxydConfig> {
    let settings = ::config::Config::builder()
        .add_source(::config::File::from(path))
        .add_source(::config::Environment::with_prefix("ROXYD").try_parsing(true))
        .build()
        .with_context(|| format!("Failed to load config from: {}", path.display()))?;

    settings
        .try_deserialize()
        .with_context(|| format!("Failed to parse config: {}", path.display()))
}

fn log_config_status(config: &RoxydConfig) {
    tracing::info!("roxyd started");
    tracing::info!("Manager address: {}", config.manager_address);
    log_quic_config(&config.quic);
    log_mtls_config(&config.mtls);
}

fn log_quic_config(quic: &QuicConfig) {
    tracing::info!(
        "QUIC config: bind_address={}, idle_timeout_ms={}",
        quic.bind_address,
        quic.idle_timeout_ms
    );
}

fn log_mtls_config(mtls: &MtlsConfig) {
    tracing::debug!(
        "mTLS config: cert_path={:?}, key_path={:?}, ca_cert_path={:?}",
        mtls.cert_path,
        mtls.key_path,
        mtls.ca_cert_path
    );
}

#[tokio::main]
async fn main() {
    let _guard = match init_tracing() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Failed to initialize tracing: {e}");
            process::exit(1);
        }
    };

    let args = Args::parse();

    let config = match load_config(&args.config) {
        Ok(cfg) => {
            tracing::info!("Loaded config from: {:?}", args.config);
            cfg
        }
        Err(e) => {
            tracing::error!("Failed to load config: {e}");
            eprintln!("Failed to load config from {}: {e}", args.config.display());
            process::exit(1);
        }
    };

    log_config_status(&config);

    tracing::info!("roxyd is running (skeleton mode - no protocol handlers active)");

    // Skeleton: no protocol handlers yet
    std::future::pending::<()>().await;
}
