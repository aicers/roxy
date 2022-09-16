mod hwinfo;
mod ifconfig;
mod ntp;
mod services;
mod sshd;
mod syslog;
pub(crate) mod task;
mod ufw;

use super::common::{run_command_output, Nic, NicOutput, SubCommand, DEFAULT_PATH_ENV};
use anyhow::{anyhow, Result};
use std::process::Command;

/// Runs linux command
/// # Errors
/// * get error code from executed command
fn run_command(cmd: &str, path: Option<&[&str]>, args: &[&str]) -> Result<bool> {
    let mut cmd = Command::new(cmd);
    let val = if let Some(path) = path {
        let mut temp = DEFAULT_PATH_ENV.to_string();
        for p in path {
            temp.push(':');
            temp.push_str(p);
        }
        temp
    } else {
        DEFAULT_PATH_ENV.to_string()
    };
    cmd.env("PATH", &val);
    for arg in args {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }

    match cmd.status() {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(anyhow!("{}", e)),
    }
}
