mod interface;
mod services;

use anyhow::{anyhow, Result};
pub use interface::{Nic, NicOutput};
use serde::{Deserialize, Serialize};
pub use services::waitfor_up;

/// Types of command to node.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum Node {
    Hostname(SubCommand),
    Interface(SubCommand),
    Ntp(SubCommand),
    PowerOff,
    Reboot,
    GracefulReboot,
    GracefulPowerOff,
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
        match bincode::serde::encode_to_vec(&cmd, bincode::config::legacy()) {
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
