mod interface;
mod services;

use anyhow::{anyhow, Result};
pub use interface::{Nic, NicOutput};
use serde::{Deserialize, Serialize};
pub use services::waitfor_up;
use std::process::Command;

pub const DEFAULT_PATH_ENV: &str = "/usr/sbin:/usr/bin:/sbin:/bin:/usr/local/aice/bin";

/// Types of command to node.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum Node {
    Hostname(SubCommand),
    Interface(SubCommand),
    Ntp(SubCommand),
    PowerOff,
    Reboot,
    Service(SubCommand),
    Sshd(SubCommand),
    Syslog(SubCommand),
    Ufw(SubCommand),
    Version(SubCommand),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeRequest {
    /// command
    pub kind: Node,
    /// command arguments
    pub arg: Vec<u8>,
}

impl NodeRequest {
    /// # Arguments
    ///
    /// * cmd<T>: command arguments. T: type of arguments
    ///
    /// # Errors
    ///
    /// * If serialization of arguments fails, then an error is returned.
    pub fn new<T>(kind: Node, cmd: T) -> Result<Self>
    where
        T: Serialize,
    {
        match bincode::serialize(&cmd) {
            Ok(arg) => Ok(NodeRequest { kind, arg }),
            Err(e) => Err(anyhow!("Error: {}", e)),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum SubCommand {
    Add,
    Delete,
    Disable,
    Enable,
    Get,
    Init,
    List,
    Set,
    SetOsVersion,
    SetProductVersion,
    Status,
    Update,
}

/// Runs a linux command and returns its output.
#[must_use]
pub fn run_command_output(cmd: &str, path: Option<&[&str]>, args: &[&str]) -> Option<String> {
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
        cmd.arg(arg);
    }
    if let Ok(output) = cmd.output() {
        if output.status.success() {
            return Some(String::from_utf8_lossy(&output.stdout).into_owned());
        }
    }
    None
}
