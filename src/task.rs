use crate::{
    hwinfo,
    ifconfig::{self, NicOutput},
    ntp, sshd, syslog, ufw,
};
use anyhow::{anyhow, Result};
use chrono::Local;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;

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

#[derive(Debug, Deserialize)]
pub enum Task {
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

impl Task {
    fn parse<T>(&self) -> Result<T>
    where
        T: serde::de::DeserializeOwned + std::fmt::Debug,
    {
        match self {
            Task::Hostname { cmd: _, arg }
            | Task::Interface { cmd: _, arg }
            | Task::Ntp { cmd: _, arg }
            | Task::Service { cmd: _, arg }
            | Task::Sshd { cmd: _, arg }
            | Task::Syslog { cmd: _, arg }
            | Task::Ufw { cmd: _, arg }
            | Task::Version { cmd: _, arg } => {
                match bincode::deserialize::<T>(&base64::decode(arg)?) {
                    Ok(r) => {
                        log_debug(&format!("arg={:?}", r));
                        Ok(r)
                    }
                    Err(e) => Err(anyhow!("fail to parse argument. {}", e)),
                }
            }
            _ => Err(anyhow!(ERR_INVALID_COMMAND)),
        }
    }
}

pub type ExecResult = std::result::Result<String, &'static str>;
pub const OKAY: &str = "Ok";
pub const ERR_INVALID_COMMAND: &str = "invalid command";
const ERR_FAIL: &str = "fail";
const ERR_MESSAGE_TOO_LONG: &str = "message too long";
const ERR_PARSE_FAIL: &str = "fail to serialize response message";

impl Task {
    /// # Errors
    /// * unsupported command
    /// * got error from the executed command
    pub fn execute(&self) -> ExecResult {
        log_debug(&format!("task {:?}", self));
        match self {
            Task::DiskUsage(_) => self.diskusage(),
            #[cfg(any(target_os = "linux"))]
            Task::PowerOff(_) => self.poweroff(),
            #[cfg(any(target_os = "linux"))]
            Task::Reboot(_) => self.reboot(),
            Task::Hostname { cmd, arg: _ } => self.hostname(*cmd),
            Task::Interface { cmd, arg: _ } => self.interface(*cmd),
            Task::Ntp { cmd, arg: _ } => self.ntp(*cmd),
            Task::Sshd { cmd, arg: _ } => self.sshd(*cmd),
            Task::Syslog { cmd, arg: _ } => self.syslog(*cmd),
            Task::Ufw { cmd, arg: _ } => self.ufw(*cmd),
            Task::Uptime(_) => self.uptime(),
            Task::Version { cmd, arg: _ } => self.version(*cmd),
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    #[cfg(any(target_os = "linux"))]
    fn reboot(&self) -> ExecResult {
        nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_AUTOBOOT)
            .map_err(|_| ERR_INVALID_COMMAND)?;
        response(self, OKAY)
    }

    #[cfg(any(target_os = "linux"))]
    fn poweroff(&self) -> ExecResult {
        nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_POWER_OFF)
            .map_err(|_| ERR_INVALID_COMMAND)?;
        response(self, OKAY)
    }

    /// Get, add, delete, enable, disable, status for ufw
    /// # Return
    /// * Option<Vec<(String, String, String, Option<String>, Option<String>)>>: Get command.
    ///   Vec<(Action, From, To, Protocol, Interface)>
    /// * OKAY: Get, Delete, Disable, Enable command
    /// * true/false: Status command
    ///
    /// # Errors
    /// * fail to execute command
    /// * unknown subcommand or invalid argument
    fn ufw(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                let ret = ufw::get().map_err(|_| ERR_FAIL)?;
                response(self, ret)
            }
            SubCommand::Add => {
                let args = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                if ufw::add(&args).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Delete => {
                let args = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                if ufw::delete(&args).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Disable => {
                if ufw::disable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Enable => {
                if ufw::enable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Status => response(self, ufw::is_active()),
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    /// Get disk usage for mount point "/data"
    /// # Return
    /// * (String, String, String, String): (mount point, total size, used size, used rate)
    ///
    /// # Errors
    /// * fail to get disk usage for "/data" directory.
    fn diskusage(&self) -> ExecResult {
        let ret = hwinfo::diskusage().map_err(|_| ERR_FAIL)?.ok_or(ERR_FAIL)?;
        response(self, ret)
    }

    /// Get uptime
    /// # Return
    /// * String: boot up time
    ///
    /// # Errors
    /// * fail to get uptime
    fn uptime(&self) -> ExecResult {
        let ret = hwinfo::uptime().ok_or(ERR_FAIL)?;
        response(self, ret)
    }

    /// Get or Set version for OS and Product
    /// # Return
    /// * (String, String): (OS version, Product Version)
    ///
    /// # Errors
    /// * fail to set version
    /// * unknown subcommand or invalid argument
    fn version(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => response(self, hwinfo::get_version()),
            SubCommand::SetOsVersion | SubCommand::SetProductVersion => {
                let arg = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if hwinfo::set_version(cmd, &arg).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    /// Get or Set remote syslog servers
    /// # Return
    /// * OKAY: Init, Set command. success to execute command
    /// * Option<Vec<(String, String, String)>>: Get command.
    ///   None if remote server addresses are not exist, else (facility, proto, addr) list
    ///
    /// # Errors
    /// * fail to execute command
    /// * unknown subcommand or invalid argument
    fn syslog(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                let ret = syslog::get().map_err(|_| ERR_FAIL)?;
                response(self, ret)
            }
            SubCommand::Init => {
                if syslog::set(&None).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let remote_addrs = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;

                if syslog::set(&Some(remote_addrs)).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    /// Get or Set hostname
    /// # Return
    /// * OKAY: Set command. Success to execute command
    /// * String: Get command. Hostname
    ///
    /// # Errors
    /// * fail to execute comand
    /// * unknown subcommand or invalid argument
    fn hostname(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                if let Ok(host) = hostname::get() {
                    response(self, host.to_string_lossy().to_string())
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let hostname = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if hostname::set(&hostname).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // TODO: simplify interface configuration for Get command
    /// Manage Nic setting
    /// # Return
    /// * OKAY: all commands except Get and List. Success to execute command
    /// * Option<Vec<(String, Nic)>>: Get command. Interface name and it's configuration.
    /// * Vec<String>: List command. Interface names list
    ///
    /// # Errors
    /// * fail to execute command
    /// * unknown subcommand or invalid argument
    fn interface(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Delete => {
                let (ifname, nic_output) = self
                    .parse::<(String, NicOutput)>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                if ifconfig::delete(&ifname, &nic_output).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Get => {
                let arg = self
                    .parse::<Option<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                match ifconfig::get(&arg) {
                    Ok(ret) => response(self, ret),
                    Err(_) => Err(ERR_FAIL),
                }
            }
            SubCommand::Init => {
                let ifname = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if ifconfig::init(&ifname).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::List => {
                if let Ok(arg) = self.parse::<Option<String>>() {
                    response(self, ifconfig::get_interface_names(&arg))
                } else {
                    Err(ERR_INVALID_COMMAND)
                }
            }
            SubCommand::Set => {
                let (ifname, nic_output) = self
                    .parse::<(String, NicOutput)>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                //for (ifname, nic_output) in ifs {
                if ifconfig::set(&ifname, &nic_output).is_err() {
                    return Err(ERR_FAIL);
                }
                //}
                response(self, OKAY)
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    /// Get or set sshd port number
    /// # Return
    /// * u16: Get command. Port number
    ///
    /// # Errors
    /// * fail to execute command
    /// * unknown subcommand or invalid argument
    fn sshd(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                if let Ok(port) = sshd::get() {
                    response(self, port)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let port = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if sshd::set(&port).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    /// Get, set, enable, disable, status
    /// # Return
    /// * OKAY: Disable, Enable, Set command. Success to execute command
    /// * Option<Vec<String>>: Get command. NTP server list
    /// * true/false: Status command.
    ///
    /// # Errors
    /// * fail to execute command
    /// * unknown subcommand or invalid argument
    fn ntp(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                if let Ok(ret) = ntp::get() {
                    response(self, ret)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Disable => {
                if ntp::disable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Enable => {
                if ntp::enable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let servers = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;

                if ntp::set(&servers).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Status => response(self, ntp::is_active()),
            _ => Err(ERR_INVALID_COMMAND),
        }
    }
}

/// make response message. max size is u32 bit long.
/// # Errors
/// * message size is over 64k
/// * fail to serialize input
fn response<I>(taskcode: &Task, input: I) -> ExecResult
where
    I: Serialize,
{
    if let Ok(message) = bincode::serialize(&input) {
        if u32::try_from(message.len()).is_err() {
            log::error!("reponse is too long. Task: {:?}", taskcode);
            Err(ERR_MESSAGE_TOO_LONG)
        } else {
            Ok(base64::encode(message))
        }
    } else {
        log::error!("failed to serialize response message. Task: {:?}", taskcode);
        Err(ERR_PARSE_FAIL)
    }
}

/// DEBUG logging
// TODO: define the full path for roxy.log file
pub fn log_debug(msg: &str) {
    if let Ok(mut writer) = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(true)
        .open("roxy.log")
    {
        let _r = writeln!(writer, "{:?}: {}", Local::now(), msg);
    }
}
