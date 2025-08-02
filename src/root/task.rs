use std::process::Command;

use anyhow::{anyhow, Result};
use data_encoding::BASE64;
use serde::{Deserialize, Serialize};

use super::{NicOutput, SubCommand};
use crate::root;

#[derive(Debug, Deserialize, Serialize)]
pub(crate) enum Task {
    Hostname { cmd: SubCommand, arg: String },
    Interface { cmd: SubCommand, arg: String },
    Ntp { cmd: SubCommand, arg: String },
    PowerOff(String),
    Reboot(String),
    GracefulReboot(String),
    GracefulPowerOff(String),
    Service { cmd: SubCommand, arg: String },
    Sshd { cmd: SubCommand, arg: String },
    Syslog { cmd: SubCommand, arg: String },
    Ufw { cmd: SubCommand, arg: String },
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
            | Task::Version { cmd: _, arg } => {
                let decoded = bincode::serde::decode_from_slice::<T, _>(
                    &BASE64.decode(arg.as_bytes())?,
                    bincode::config::legacy(),
                );
                match decoded {
                    Ok((r, _)) => {
                        tracing::info!("arg={r:?}");
                        Ok(r)
                    }
                    Err(e) => Err(anyhow!("fail to parse argument. {}", e)),
                }
            }
            _ => Err(anyhow!(ERR_INVALID_COMMAND)),
        }
    }
}

pub(crate) type ExecResult = std::result::Result<String, &'static str>;
pub(crate) const OKAY: &str = "Ok";
pub(crate) const ERR_INVALID_COMMAND: &str = "invalid command";
const ERR_FAIL: &str = "fail";
const ERR_MESSAGE_TOO_LONG: &str = "message too long";
const ERR_PARSE_FAIL: &str = "fail to serialize response message";

impl Task {
    // # Errors
    //
    // * unsupported command
    // * got error from the executed command
    pub fn execute(&self) -> ExecResult {
        tracing::info!("task {self:?}");
        match self {
            #[cfg(target_os = "linux")]
            Task::PowerOff(_) => self.poweroff(),
            #[cfg(target_os = "linux")]
            Task::Reboot(_) => self.reboot(),
            Task::GracefulReboot(_) => self.graceful_reboot(),
            Task::GracefulPowerOff(_) => self.graceful_poweroff(),
            Task::Hostname { cmd, arg: _ } => self.hostname(*cmd),
            Task::Interface { cmd, arg: _ } => self.interface(*cmd),
            Task::Ntp { cmd, arg: _ } => self.ntp(*cmd),
            Task::Sshd { cmd, arg: _ } => self.sshd(*cmd),
            Task::Syslog { cmd, arg: _ } => self.syslog(*cmd),
            Task::Version { cmd, arg: _ } => self.version(*cmd),
            Task::Service { cmd, arg: _ } => self.service(*cmd),
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    #[cfg(target_os = "linux")]
    fn reboot(&self) -> ExecResult {
        nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_AUTOBOOT)
            .map_err(|_| ERR_INVALID_COMMAND)?;
        response(self, OKAY)
    }

    #[cfg(target_os = "linux")]
    fn poweroff(&self) -> ExecResult {
        nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_POWER_OFF)
            .map_err(|_| ERR_INVALID_COMMAND)?;
        response(self, OKAY)
    }

    fn graceful_reboot(&self) -> ExecResult {
        #[cfg(target_os = "linux")]
        let cmd = "reboot";
        #[cfg(target_os = "macos")]
        let cmd = "sudo";

        #[cfg(target_os = "linux")]
        let result = Command::new(cmd).spawn();
        #[cfg(target_os = "macos")]
        let result = Command::new(cmd).args(["reboot"]).spawn();

        match result {
            Ok(_) => response(self, OKAY),
            Err(e) => {
                tracing::debug!("Failed to execute graceful reboot: {e}");
                Err(ERR_FAIL)
            }
        }
    }

    fn graceful_poweroff(&self) -> ExecResult {
        #[cfg(target_os = "linux")]
        let cmd = "poweroff";
        #[cfg(target_os = "macos")]
        let cmd = "sudo";

        #[cfg(target_os = "linux")]
        let result = Command::new(cmd).spawn();
        #[cfg(target_os = "macos")]
        let result = Command::new(cmd).args(["shutdown", "-h", "now"]).spawn();

        match result {
            Ok(_) => response(self, OKAY),
            Err(e) => {
                tracing::debug!("Failed to execute graceful poweroff: {e}");
                Err(ERR_FAIL)
            }
        }
    }

    // Gets or sets version for OS and Product
    //
    // # Return
    // * (String, String): (OS version, Product Version)
    //
    // # Errors
    // * fail to set version
    // * unknown subcommand or invalid argument
    fn version(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::SetOsVersion | SubCommand::SetProductVersion => {
                let arg = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if crate::root::hwinfo::set_version(cmd, &arg).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // Start, stop, status(is-active), restart(update) the services or get status
    fn service(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Disable | SubCommand::Enable | SubCommand::Status | SubCommand::Update => {
                let service = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                match root::services::service_control(&service, cmd) {
                    Ok(r) => response(self, r),
                    _ => Err(ERR_FAIL),
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // Gets or sets or restarts remote syslog servers
    //
    // # Return
    //
    // * OKAY: Init, Set command. success to execute command
    // * Option<Vec<(String, String, String)>>: Get command.
    //   None if remote server addresses are not exist, else (facility, proto, addr) list
    //
    // # Errors
    //
    // * fail to execute command
    // * unknown subcommand or invalid argument
    fn syslog(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                let ret = root::syslog::get().map_err(|_| ERR_FAIL)?;
                response(self, ret)
            }
            SubCommand::Init => {
                if root::syslog::set(None).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let remote_addrs = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;

                if root::syslog::set(Some(&remote_addrs)).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Enable => {
                if root::syslog::start().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // Gets or sets hostname
    //
    // # Return
    //
    // * OKAY: Set command. Success to execute command
    // * String: Get command. Hostname
    //
    // # Errors
    //
    // * fail to execute comand
    // * unknown subcommand or invalid argument
    fn hostname(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => response(self, roxy::hostname()),
            SubCommand::Set => {
                let hostname = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if hostname::set(hostname).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // TODO: simplify interface configuration for Get command
    // Manages Nic setting
    //
    // # Return
    //
    // * OKAY: all commands except Get and List. Success to execute command
    // * Option<Vec<(String, Nic)>>: Get command. Interface name and it's configuration.
    // * Vec<String>: List command. Interface names list
    //
    // # Errors
    //
    // * fail to execute command
    // * unknown subcommand or invalid argument
    fn interface(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Delete => {
                let (ifname, nic_output) = self
                    .parse::<(String, NicOutput)>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                if root::ifconfig::delete(&ifname, &nic_output).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Get => {
                let arg = self
                    .parse::<Option<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                match root::ifconfig::get(arg.as_ref()) {
                    Ok(ret) => response(self, ret),
                    Err(_) => Err(ERR_FAIL),
                }
            }
            SubCommand::Init => {
                let ifname = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if root::ifconfig::init(&ifname).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::List => {
                if let Ok(arg) = self.parse::<Option<String>>() {
                    response(self, root::ifconfig::get_interface_names(arg.as_ref()))
                } else {
                    Err(ERR_INVALID_COMMAND)
                }
            }
            SubCommand::Set => {
                let (ifname, nic_output) = self
                    .parse::<(String, NicOutput)>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;
                if root::ifconfig::set(&ifname, &nic_output).is_err() {
                    return Err(ERR_FAIL);
                }
                response(self, OKAY)
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // Gets or sets or restarts sshd
    //
    // # Return
    //
    // * u16: Get command. Port number
    //
    // # Errors
    //
    // * fail to execute command
    // * unknown subcommand or invalid argument
    fn sshd(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                if let Ok(port) = root::sshd::get() {
                    response(self, port)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let port = self.parse::<String>().map_err(|_| ERR_INVALID_COMMAND)?;
                if root::sshd::set(&port).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Enable => {
                if root::sshd::start().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            _ => Err(ERR_INVALID_COMMAND),
        }
    }

    // # Return
    //
    // * OKAY: Disable, Enable, Set command. Success to execute command
    // * Option<Vec<String>>: Get command. NTP server list
    // * true/false: Status command.
    //
    // # Errors
    //
    // * fail to execute command
    // * unknown subcommand or invalid argument
    fn ntp(&self, cmd: SubCommand) -> ExecResult {
        match cmd {
            SubCommand::Get => {
                if let Ok(ret) = root::ntp::get() {
                    response(self, ret)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Disable => {
                if root::ntp::disable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Enable => {
                if root::ntp::enable().is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Set => {
                let servers = self
                    .parse::<Vec<String>>()
                    .map_err(|_| ERR_INVALID_COMMAND)?;

                if root::ntp::set(&servers).is_ok() {
                    response(self, OKAY)
                } else {
                    Err(ERR_FAIL)
                }
            }
            SubCommand::Status => response(self, root::ntp::is_active()),
            _ => Err(ERR_INVALID_COMMAND),
        }
    }
}

// Makes response message. max size is u32 bit long.
//
// # Errors
//
// * message size is over 64k
// * fail to serialize input
fn response<I>(taskcode: &Task, input: I) -> ExecResult
where
    I: Serialize,
{
    if let Ok(message) = bincode::serde::encode_to_vec(&input, bincode::config::legacy()) {
        if u32::try_from(message.len()).is_err() {
            tracing::error!("reponse is too long. Task: {taskcode:?}");
            Err(ERR_MESSAGE_TOO_LONG)
        } else {
            Ok(BASE64.encode(&message))
        }
    } else {
        tracing::error!("failed to serialize response message. Task: {taskcode:?}");
        Err(ERR_PARSE_FAIL)
    }
}
