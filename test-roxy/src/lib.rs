use anyhow::{anyhow, Result};
//use chrono::Local;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::io::Write;
use std::process::{Command, Stdio};

/// Response message from Roxy
#[derive(Deserialize, Debug)]
pub enum TaskResult {
    Ok(String),
    Err(String),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NicOutput {
    addresses: Option<Vec<String>>,
    dhcp4: Option<bool>,
    gateway4: Option<String>,
    nameservers: Option<Vec<String>>,
}

impl NicOutput {
    #[must_use]
    pub fn new(
        addresses: Option<Vec<String>>,
        dhcp4: Option<bool>,
        gateway4: Option<String>,
        nameservers: Option<Vec<String>>,
    ) -> Self {
        NicOutput {
            addresses,
            dhcp4,
            gateway4,
            nameservers,
        }
    }
}

/// Json message to Roxy
#[derive(Debug, Deserialize, Serialize, Clone)]
#[allow(dead_code)]
pub enum RoxyTask {
    DiskUsage(String),
    Hostname { cmd: SubCommand, arg: String },
    Interface { cmd: SubCommand, arg: String },
    Ntp { cmd: SubCommand, arg: String },
    PowerOff(String),
    Reboot(String),
    Service { cmd: SubCommand, arg: String },
    Sshd { cmd: SubCommand, arg: String },
    Syslog { cmd: SubCommand, arg: String },
    Ufw { cmd: SubCommand, arg: String },
    Uptime(String),
    Version { cmd: SubCommand, arg: String },
}

/// Types of command to process
#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
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

/// Types of command to node
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
    //pub seq: i64,
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

    /// # Errors
    /// * fail to serialize message
    /// * fail to parse to json
    pub fn roxy_task(&self) -> Result<String> {
        let arg = base64::encode(&self.arg);
        let task = match self.kind {
            Node::DiskUsage => RoxyTask::DiskUsage(arg),
            Node::Hostname(cmd) => RoxyTask::Hostname { cmd, arg },
            Node::Interface(cmd) => RoxyTask::Interface { cmd, arg },
            Node::Ntp(cmd) => RoxyTask::Ntp { cmd, arg },
            Node::PowerOff => RoxyTask::PowerOff(arg),
            Node::Reboot => RoxyTask::Reboot(arg),
            Node::Service(cmd) => RoxyTask::Service { cmd, arg },
            Node::Sshd(cmd) => RoxyTask::Sshd { cmd, arg },
            Node::Syslog(cmd) => RoxyTask::Syslog { cmd, arg },
            Node::Ufw(cmd) => RoxyTask::Ufw { cmd, arg },
            Node::Uptime => RoxyTask::Uptime(arg),
            Node::Version(cmd) => RoxyTask::Version { cmd, arg },
        };
        serde_json::to_string(&task).map_err(|_| anyhow!("fail to parse node request to json"))
    }

    pub fn debug<T>(&self)
    where
        T: DeserializeOwned + Debug,
    {
        if let Ok(value) = bincode::deserialize::<T>(&self.arg) {
            println!("DEBUG: Task = {:?}, arg = {:?}", self.kind, value);
        }
    }
}

// TODO: fix the exact path to "roxy"
/// # Errors
/// * fail to spawn roxy
/// * fail to write command to roxy
/// * invalid json syntax in response message
/// * base64 decode error for reponse message
/// * received executtion error from roxy
pub fn run_roxy<T>(args: &str) -> Result<T>
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

    if let Some(child_stdin) = child.stdin.take().as_mut() {
        write!(child_stdin, "{}", args)?;
        // Close stdin to finish and avoid indefinite blocking
        // drop(child_stdin);
    } else {
        return Err(anyhow!("fail to execute command"));
    }

    let output = child.wait_with_output()?;
    let output = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<TaskResult>(&output) {
        Ok(TaskResult::Ok(x)) => {
            let decoded = base64::decode(&x).map_err(|_| anyhow!("fail to decode response."))?;
            Ok(bincode::deserialize::<T>(&decoded)?)
        }
        Ok(TaskResult::Err(x)) => Err(anyhow!("{}", x)),
        Err(e) => Err(anyhow!("fail to parse response. {}", e)),
    }
}
