mod hwinfo;
mod ifconfig;
mod ntp;
mod sshd;
mod syslog;
pub mod task;
mod ufw;

use anyhow::{anyhow, Result};
pub use ifconfig::NicOutput;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::process::{Command, Stdio};
pub use task::{SubCommand, Task};

const DEFAULT_PATH_ENV: &str = "/usr/sbin:/usr/bin:/sbin:/bin:/usr/local/aice/bin";

/// Run linux command
/// # Errors
/// * get error code from executed command
pub fn run_command(cmd: &str, path: Option<&[&str]>, args: &[&str]) -> Result<bool> {
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

/// Run linux command and return it's output
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

/// Response message from Roxy to caller
#[derive(Deserialize, Debug)]
pub enum TaskResult {
    Ok(String),
    Err(String),
}

/// Types of command to node.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub enum Node {
    DiskUsage,
    Hostname(SubCommand),
    Interface(SubCommand),
    Ntp(SubCommand),
    PowerOff,
    Reboot,
    Service(SubCommand),
    Sshd(SubCommand),
    Syslog(SubCommand),
    Ufw(SubCommand),
    Uptime,
    Version(SubCommand),
}

/// Request message structure between nodes
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeRequest {
    /// sequence number to distinguish each request for multiple users
    //seq: i64,
    /// destination hostname
    pub host: String,
    /// destination process name
    pub process: String,
    /// command
    pub kind: Node,
    /// command arguments
    pub arg: Vec<u8>,
}

impl NodeRequest {
    /// # Arguments
    /// * cmd<T>: command arguments. T: type of arguments
    ///
    /// # Errors
    /// * fail to serialize command
    pub fn new<T>(host: &str, process: &str, kind: Node, cmd: T) -> Result<Self>
    where
        T: Serialize,
    {
        //let seq = Local::now().timestamp_nanos();
        match bincode::serialize(&cmd) {
            Ok(arg) => Ok(NodeRequest {
                //seq,
                host: host.to_string(),
                process: process.to_string(),
                kind,
                arg,
            }),
            Err(e) => Err(anyhow!("Error: {}", e)),
        }
    }

    /// Converts `NodeRequest` to `Task`.
    #[must_use]
    pub fn to_task(&self) -> Task {
        let arg = base64::encode(&self.arg);
        match self.kind {
            Node::DiskUsage => Task::DiskUsage(arg),
            Node::Hostname(cmd) => Task::Hostname { cmd, arg },
            Node::Interface(cmd) => Task::Interface { cmd, arg },
            Node::Ntp(cmd) => Task::Ntp { cmd, arg },
            Node::PowerOff => Task::PowerOff(arg),
            Node::Reboot => Task::Reboot(arg),
            Node::Service(cmd) => Task::Service { cmd, arg },
            Node::Sshd(cmd) => Task::Sshd { cmd, arg },
            Node::Syslog(cmd) => Task::Syslog { cmd, arg },
            Node::Ufw(cmd) => Task::Ufw { cmd, arg },
            Node::Uptime => Task::Uptime(arg),
            Node::Version(cmd) => Task::Version { cmd, arg },
        }
    }

    pub fn debug<T>(&self)
    where
        T: DeserializeOwned + std::fmt::Debug,
    {
        if let Ok(value) = bincode::deserialize::<T>(&self.arg) {
            println!("DEBUG: Task = {:?}, arg = {:?}", self.kind, value);
        }
    }
}

// TODO: fix the exact path to "roxy"
///
/// # Errors
///
/// * Failure to spawn roxy
/// * Failure to write command to roxy
/// * Invalid json syntax in response message
/// * base64 decode error for reponse message
/// * Received execution error from roxy
pub fn run_roxy<T>(task: Task) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut child = Command::new("roxy")
        .env(
            "PATH",
            "/usr/local/aice/bin:/usr/sbin:/usr/bin:/sbin:/bin:.",
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(child_stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            serde_json::to_writer(child_stdin, &task).expect("`Task` should serialize to JSON");
        });
    } else {
        return Err(anyhow!("failed to execute roxy"));
    }

    let output = child.wait_with_output()?;
    match serde_json::from_reader::<&[u8], TaskResult>(&output.stdout) {
        Ok(TaskResult::Ok(x)) => {
            let decoded = base64::decode(&x).map_err(|_| anyhow!("fail to decode response."))?;
            Ok(bincode::deserialize::<T>(&decoded)?)
        }
        Ok(TaskResult::Err(x)) => Err(anyhow!("{}", x)),
        Err(e) => Err(anyhow!("fail to parse response. {}", e)),
    }
}
