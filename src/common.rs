use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, process::Command};

pub const DEFAULT_PATH_ENV: &str = "/usr/sbin:/usr/bin:/sbin:/bin:/usr/local/aice/bin";

/// Types of command to node.
#[derive(Debug, Deserialize, Serialize, Clone)]
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

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Nic {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub addresses: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp4: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway4: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nameservers: Option<HashMap<String, Vec<String>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub optional: Option<bool>,
}

impl fmt::Display for Nic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Ok(s) = serde_yaml::to_string(self) {
            write!(f, "{}", s)
        } else {
            Ok(())
        }
    }
}

impl Nic {
    #[must_use]
    pub fn new(
        addresses: Option<Vec<String>>,
        dhcp4: Option<bool>,
        gateway4: Option<String>,
        nameservers: Option<HashMap<String, Vec<String>>>,
        optional: Option<bool>,
    ) -> Self {
        Nic {
            addresses,
            dhcp4,
            gateway4,
            nameservers,
            optional,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NicOutput {
    pub addresses: Option<Vec<String>>,
    pub dhcp4: Option<bool>,
    pub gateway4: Option<String>,
    pub nameservers: Option<Vec<String>>,
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

    #[must_use]
    pub fn to(&self) -> Nic {
        let nameservers = if let Some(nm) = &self.nameservers {
            let mut m = HashMap::new();
            m.insert("addresses".to_string(), nm.clone());
            m.insert("search".to_string(), Vec::new());
            Some(m)
        } else {
            None
        };
        Nic {
            addresses: self.addresses.clone(),
            dhcp4: self.dhcp4,
            gateway4: self.gateway4.clone(),
            nameservers,
            optional: None,
        }
    }

    #[must_use]
    pub fn from(nic: &Nic) -> Self {
        let nameservers = {
            if let Some(nm) = &nic.nameservers {
                nm.get("addresses").cloned()
            } else {
                None
            }
        };
        NicOutput {
            addresses: nic.addresses.clone(),
            dhcp4: nic.dhcp4,
            gateway4: nic.gateway4.clone(),
            nameservers,
        }
    }
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
