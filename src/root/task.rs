use std::process::Command;

use anyhow::{Result, anyhow};
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
                match bincode::deserialize::<T>(&BASE64.decode(arg.as_bytes())?) {
                    Ok(r) => {
                        tracing::info!("arg={r:?}");
                        Ok(r)
                    }
                    Err(e) => Err(anyhow!("fail to parse argument. {e}")),
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
    if let Ok(message) = bincode::serialize(&input) {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a base64-encoded bincode serialized value for use as Task args.
    fn encode_arg<T: Serialize>(value: &T) -> String {
        let bytes = bincode::serialize(value).expect("bincode serialize should succeed");
        BASE64.encode(&bytes)
    }

    // Task::parse decoding tests
    //
    // The parse method decodes the arg field from base64+bincode format.
    // It only works for Task variants that have cmd/arg fields (Hostname,
    // Interface, Ntp, Service, Sshd, Syslog, Version). PowerOff, Reboot,
    // GracefulReboot, GracefulPowerOff, and Ufw return ERR_INVALID_COMMAND.

    #[test]
    fn parse_decodes_string_arg() {
        let value = "test-hostname".to_string();
        let task = Task::Hostname {
            cmd: SubCommand::Set,
            arg: encode_arg(&value),
        };

        let parsed: String = task.parse().expect("parse should succeed");
        assert_eq!(parsed, value);
    }

    #[test]
    fn parse_decodes_vec_string_arg() {
        let value = vec!["server1.example.com".to_string(), "server2.example.com".to_string()];
        let task = Task::Ntp {
            cmd: SubCommand::Set,
            arg: encode_arg(&value),
        };

        let parsed: Vec<String> = task.parse().expect("parse should succeed");
        assert_eq!(parsed, value);
    }

    #[test]
    fn parse_decodes_option_string_none() {
        let value: Option<String> = None;
        let task = Task::Interface {
            cmd: SubCommand::Get,
            arg: encode_arg(&value),
        };

        let parsed: Option<String> = task.parse().expect("parse should succeed");
        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_decodes_option_string_some() {
        let value: Option<String> = Some("eth0".to_string());
        let task = Task::Interface {
            cmd: SubCommand::List,
            arg: encode_arg(&value),
        };

        let parsed: Option<String> = task.parse().expect("parse should succeed");
        assert_eq!(parsed, Some("eth0".to_string()));
    }

    #[test]
    fn parse_decodes_tuple_with_nic_output() {
        let nic = NicOutput::new(
            Some(vec!["192.168.1.100/24".to_string()]),
            Some(false),
            Some("192.168.1.1".to_string()),
            Some(vec!["8.8.8.8".to_string()]),
        );
        let value = ("eth0".to_string(), nic);
        let task = Task::Interface {
            cmd: SubCommand::Set,
            arg: encode_arg(&value),
        };

        let parsed: (String, NicOutput) = task.parse().expect("parse should succeed");
        assert_eq!(parsed.0, "eth0");
        assert_eq!(parsed.1.addresses, Some(vec!["192.168.1.100/24".to_string()]));
        assert_eq!(parsed.1.dhcp4, Some(false));
        assert_eq!(parsed.1.gateway4, Some("192.168.1.1".to_string()));
        assert_eq!(parsed.1.nameservers, Some(vec!["8.8.8.8".to_string()]));
    }

    #[test]
    fn parse_decodes_empty_vec() {
        let value: Vec<String> = vec![];
        let task = Task::Syslog {
            cmd: SubCommand::Set,
            arg: encode_arg(&value),
        };

        let parsed: Vec<String> = task.parse().expect("parse should succeed");
        assert!(parsed.is_empty());
    }

    #[test]
    fn parse_works_for_all_supported_variants() {
        // Tests that parse works for each Task variant that has cmd/arg fields.
        let arg = encode_arg(&"test".to_string());

        let tasks = [
            Task::Hostname {
                cmd: SubCommand::Get,
                arg: arg.clone(),
            },
            Task::Interface {
                cmd: SubCommand::Get,
                arg: arg.clone(),
            },
            Task::Ntp {
                cmd: SubCommand::Get,
                arg: arg.clone(),
            },
            Task::Service {
                cmd: SubCommand::Status,
                arg: arg.clone(),
            },
            Task::Sshd {
                cmd: SubCommand::Get,
                arg: arg.clone(),
            },
            Task::Syslog {
                cmd: SubCommand::Get,
                arg: arg.clone(),
            },
            Task::Version {
                cmd: SubCommand::SetOsVersion,
                arg,
            },
        ];

        for task in tasks {
            let parsed: String = task.parse().expect("parse should succeed for supported variant");
            assert_eq!(parsed, "test");
        }
    }

    // Invalid command path tests
    //
    // Task variants PowerOff, Reboot, GracefulReboot, GracefulPowerOff, and Ufw
    // do not support parse and return ERR_INVALID_COMMAND.

    #[test]
    fn parse_rejects_poweroff_variant() {
        let task = Task::PowerOff(String::new());
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ERR_INVALID_COMMAND));
    }

    #[test]
    fn parse_rejects_reboot_variant() {
        let task = Task::Reboot(String::new());
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ERR_INVALID_COMMAND));
    }

    #[test]
    fn parse_rejects_graceful_reboot_variant() {
        let task = Task::GracefulReboot(String::new());
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ERR_INVALID_COMMAND));
    }

    #[test]
    fn parse_rejects_graceful_poweroff_variant() {
        let task = Task::GracefulPowerOff(String::new());
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ERR_INVALID_COMMAND));
    }

    #[test]
    fn parse_rejects_ufw_variant() {
        let task = Task::Ufw {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains(ERR_INVALID_COMMAND));
    }

    #[test]
    fn parse_fails_on_invalid_base64() {
        let task = Task::Hostname {
            cmd: SubCommand::Get,
            arg: "not-valid-base64!!!".to_string(),
        };
        let result = task.parse::<String>();
        assert!(result.is_err());
    }

    #[test]
    fn parse_fails_on_invalid_bincode() {
        // Valid base64 but invalid bincode for String type
        let task = Task::Hostname {
            cmd: SubCommand::Get,
            arg: BASE64.encode(&[0xff, 0xff, 0xff, 0xff]),
        };
        let result = task.parse::<String>();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("fail to parse argument"));
    }

    #[test]
    fn parse_fails_on_type_mismatch() {
        // Encode a String but try to parse as Vec<String>
        let task = Task::Ntp {
            cmd: SubCommand::Set,
            arg: encode_arg(&"single-string".to_string()),
        };
        let result = task.parse::<Vec<String>>();
        assert!(result.is_err());
    }

    // Response encoding tests
    //
    // The response function serializes data via bincode and encodes to base64.
    // The output can be decoded back to verify the encoding.

    #[test]
    fn response_encodes_string() {
        let task = Task::Hostname {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let result = response(&task, OKAY).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: &str = bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(decoded, OKAY);
    }

    #[test]
    fn response_encodes_u16() {
        let task = Task::Sshd {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let port: u16 = 22;
        let result = response(&task, port).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: u16 = bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(decoded, 22);
    }

    #[test]
    fn response_encodes_bool() {
        let task = Task::Ntp {
            cmd: SubCommand::Status,
            arg: String::new(),
        };
        let result = response(&task, true).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: bool = bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert!(decoded);
    }

    #[test]
    fn response_encodes_option_vec_string_none() {
        let task = Task::Ntp {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let value: Option<Vec<String>> = None;
        let result = response(&task, value).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: Option<Vec<String>> =
            bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(decoded, None);
    }

    #[test]
    fn response_encodes_option_vec_string_some() {
        let task = Task::Ntp {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let value: Option<Vec<String>> = Some(vec!["ntp1.example.com".to_string()]);
        let result = response(&task, value).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: Option<Vec<String>> =
            bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(decoded, Some(vec!["ntp1.example.com".to_string()]));
    }

    #[test]
    fn response_encodes_vec_tuple() {
        let task = Task::Syslog {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let value: Option<Vec<(String, String, String)>> = Some(vec![(
            "local0".to_string(),
            "tcp".to_string(),
            "192.168.1.100:514".to_string(),
        )]);
        let result = response(&task, value).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: Option<Vec<(String, String, String)>> =
            bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(
            decoded,
            Some(vec![("local0".to_string(), "tcp".to_string(), "192.168.1.100:514".to_string())])
        );
    }

    #[test]
    fn response_encodes_interface_list() {
        let task = Task::Interface {
            cmd: SubCommand::List,
            arg: String::new(),
        };
        let value = vec!["eth0".to_string(), "eth1".to_string(), "lo".to_string()];
        let result = response(&task, value).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: Vec<String> =
            bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert_eq!(decoded, vec!["eth0", "eth1", "lo"]);
    }

    #[test]
    fn response_encodes_empty_vec() {
        let task = Task::Interface {
            cmd: SubCommand::List,
            arg: String::new(),
        };
        let value: Vec<String> = vec![];
        let result = response(&task, value).expect("response should succeed");

        let decoded_bytes = BASE64.decode(result.as_bytes()).expect("base64 decode should succeed");
        let decoded: Vec<String> =
            bincode::deserialize(&decoded_bytes).expect("bincode deserialize should succeed");
        assert!(decoded.is_empty());
    }

    // Serialization failure test
    //
    // The response function returns ERR_PARSE_FAIL when bincode serialization fails.
    // This is difficult to trigger with normal types since bincode handles most cases.
    // The ERR_MESSAGE_TOO_LONG branch requires a message > u32::MAX bytes, which is
    // impractical to test without production changes to allow smaller limits.

    #[test]
    fn response_fails_on_unserializable_type() {
        struct BadSerialize;

        impl Serialize for BadSerialize {
            fn serialize<S>(&self, _serializer: S) -> std::result::Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("intentional failure"))
            }
        }

        let task = Task::Hostname {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let result = response(&task, BadSerialize);
        assert_eq!(result, Err(ERR_PARSE_FAIL));
    }

    // Execute tests
    //
    // Most execute() branches call OS-level functions (hostname::set, systemctl,
    // file operations, reboot syscalls) that cannot be tested without production
    // changes or mocking infrastructure. The following tests cover branches that
    // return ERR_INVALID_COMMAND without side effects.

    #[test]
    fn execute_ufw_returns_invalid_command() {
        // Ufw variant is defined but execute() has no handler, falling through to Err.
        let task = Task::Ufw {
            cmd: SubCommand::Get,
            arg: String::new(),
        };
        let result = task.execute();
        assert_eq!(result, Err(ERR_INVALID_COMMAND));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn execute_poweroff_returns_invalid_command_on_non_linux() {
        // On non-Linux, PowerOff falls through to the default error branch.
        let task = Task::PowerOff(String::new());
        let result = task.execute();
        assert_eq!(result, Err(ERR_INVALID_COMMAND));
    }

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn execute_reboot_returns_invalid_command_on_non_linux() {
        // On non-Linux, Reboot falls through to the default error branch.
        let task = Task::Reboot(String::new());
        let result = task.execute();
        assert_eq!(result, Err(ERR_INVALID_COMMAND));
    }
}
