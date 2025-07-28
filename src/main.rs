mod root;

use std::fs;
use std::{
    io::{stdin, stdout},
    process,
};

use anyhow::{Context, Result};
use data_encoding::BASE64;
use root::task::{ExecResult, Task, ERR_INVALID_COMMAND};
use roxy::common::{self, Node, NodeRequest};
use tracing::level_filters::LevelFilter;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

fn init_tracing() -> Result<WorkerGuard> {
    let log_path = "/opt/clumit/log/roxy.log";
    let (layer, guard) = {
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_path)
            .with_context(|| format!("Failed to open the log file: {log_path}"))?;
        let (non_blocking, file_guard) = tracing_appender::non_blocking(file);
        (
            fmt::Layer::default()
                .with_ansi(false)
                .with_target(false)
                .with_writer(non_blocking)
                .with_filter(
                    EnvFilter::builder()
                        .with_default_directive(LevelFilter::INFO.into())
                        .from_env_lossy(),
                ),
            file_guard,
        )
    };

    tracing_subscriber::Registry::default().with(layer).init();
    Ok(guard)
}

fn main() {
    let _guard = init_tracing();

    let nr: NodeRequest = match serde_json::from_reader(stdin()) {
        Ok(nr) => nr,
        Err(err) => {
            tracing::error!("Command Error: {err}");
            if let Err(err) =
                serde_json::to_writer_pretty(stdout(), &ExecResult::Err(ERR_INVALID_COMMAND))
            {
                tracing::error!("Serialize Error: {err}");
            }
            process::exit(1);
        }
    };

    let arg = BASE64.encode(&nr.arg);
    let task = match nr.kind {
        Node::Hostname(cmd) => Task::Hostname { cmd, arg },
        Node::Interface(cmd) => Task::Interface { cmd, arg },
        Node::Ntp(cmd) => Task::Ntp { cmd, arg },
        Node::PowerOff => Task::PowerOff(arg),
        Node::Reboot => Task::Reboot(arg),
        Node::GracefulReboot => Task::GracefulReboot(arg),
        Node::GracefulPowerOff => Task::GracefulPowerOff(arg),
        Node::Service(cmd) => Task::Service { cmd, arg },
        Node::Sshd(cmd) => Task::Sshd { cmd, arg },
        Node::Syslog(cmd) => Task::Syslog { cmd, arg },
        Node::Ufw(cmd) => Task::Ufw { cmd, arg },
        Node::Version(cmd) => Task::Version { cmd, arg },
    };

    let ret = task.execute();
    if let Err(err) = serde_json::to_writer_pretty(stdout(), &ret) {
        tracing::error!("Stdout Error: {err}");
        process::exit(1);
    }
}
