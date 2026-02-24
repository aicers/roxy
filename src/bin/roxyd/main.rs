//! `roxyd` - New implementation path for QUIC/mTLS connectivity with Manager.
//!
//! This is a skeleton binary entrypoint that coexists with the legacy `roxy` binary.
//! It provides configuration loading, tracing initialization, and async runtime
//! bootstrap, but does not yet implement any review-protocol request handling.
//!
//! # Usage
//!
//! ```sh
//! cargo run --bin roxyd -- -c path/to/config.toml --cert path/to/cert.pem \
//!   --key path/to/key.pem --ca-certs path/to/ca.pem manager@192.168.1.100:4433
//! ```

mod settings;

use std::{fs, path::Path, process::ExitCode};

use anyhow::{Context, Result};
use clap::Parser;
use settings::{Args, Config, Settings};
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// Initializes tracing/logging infrastructure.
///
/// Uses a file appender with non-blocking writes for performance. The log level
/// can be controlled via the `RUST_LOG` environment variable, defaulting to INFO.
///
/// # Errors
///
/// Returns an error if the log file cannot be opened or created.
fn init_tracing(log_path: Option<&Path>) -> Result<WorkerGuard> {
    let (layer, guard) = if let Some(log_path) = log_path {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .with_context(|| format!("Failed to open the log file: {}", log_path.display()))?;
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
    } else {
        let (non_blocking, stdout_guard) = tracing_appender::non_blocking(std::io::stdout());
        let env_filter = EnvFilter::builder()
            .with_default_directive(LevelFilter::INFO.into())
            .from_env_lossy();
        (
            fmt::Layer::default()
                .with_ansi(true)
                .with_target(false)
                .with_writer(non_blocking)
                .with_filter(env_filter),
            stdout_guard,
        )
    };

    tracing_subscriber::Registry::default().with(layer).init();
    Ok(guard)
}

fn log_config_status(settings: &Settings) {
    tracing::info!("Manager server: {}", settings.manager_server);
    tracing::debug!("cert path: {}", settings.cert.display());
    tracing::debug!("key path: {}", settings.key.display());
    tracing::debug!("ca cert files: {}", settings.ca_certs.len());
    if settings.log_path().is_none() {
        tracing::info!("log_path not set, logging to stdout");
    }
}

#[tokio::main]
async fn main() -> ExitCode {
    let args = Args::parse();
    let config = match Config::load(&args.config) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let guard = match init_tracing(config.log_path.as_deref()) {
        Ok(guard) => guard,
        Err(err) => {
            eprintln!("{err}");
            return ExitCode::FAILURE;
        }
    };
    let settings = match Settings::from_args(&args, config) {
        Ok(settings) => settings,
        Err(err) => {
            tracing::error!("roxyd startup failed: {err}");
            drop(guard);
            return ExitCode::FAILURE;
        }
    };

    run(&args, &settings);
    drop(guard);
    ExitCode::SUCCESS
}

fn run(args: &Args, settings: &Settings) {
    tracing::info!("Loaded config from: {:?}", args.config);
    log_config_status(settings);

    tracing::info!("roxyd is running (skeleton mode - no protocol handlers active)");
}
