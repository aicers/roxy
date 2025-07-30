pub mod common;
mod user;

use std::process::{Command, Stdio};

use anyhow::{anyhow, Result};
pub use common::waitfor_up;
use common::{NicOutput, Node, NodeRequest, SubCommand};
use data_encoding::BASE64;
use serde::Deserialize;
pub use user::hwinfo::{uptime, version};
pub use user::process::{process_list, Process};
pub use user::usg::{resource_usage, ResourceUsage};
const FAIL_REQUEST: &str = "Failed to create a request";

/// Control services: start, stop, restart, status
///
/// # Errors
///
/// * Return error if invalid subcommand is specified
/// * Return error if target service is not registered as a systemctl service
/// * Return error if it failed to execute the command
pub fn service_control(subcmd: SubCommand, service: String) -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<String>(Node::Service(subcmd), service) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Returns a hostname.
#[must_use]
pub fn hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Sets a version for OS.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If reading or writing of an OS version file fails, then an error
///   is returned.
pub fn set_os_version(ver: String) -> Result<String> {
    if let Ok(req) = NodeRequest::new::<String>(Node::Version(SubCommand::SetOsVersion), ver) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Sets a version for product.
///
/// # Errors
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If reading or writing of a product version file fails, then an error
///   is returned.
pub fn set_product_version(ver: String) -> Result<String> {
    if let Ok(req) = NodeRequest::new::<String>(Node::Version(SubCommand::SetProductVersion), ver) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Sets a hostname.
///
/// # Errors
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If `hostname::set` fails, then an error is returned.
pub fn set_hostname(host: String) -> Result<String> {
    if let Ok(req) = NodeRequest::new::<String>(Node::Hostname(SubCommand::Set), host) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Returns tuples of (facilitiy, proto, addr) of syslog servers.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to open `/etc/rsyslog.d/50-default.conf`, then an error
///   is returned.
pub fn syslog_servers() -> Result<Option<Vec<(String, String, String)>>> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Syslog(SubCommand::Get), None) {
        run_roxy::<Option<Vec<(String, String, String)>>>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Sets syslog servers.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to open or write `/etc/rsyslog.d/50-default.conf`, then
///   an error is returned.
/// * If it fails to restart rsyslogd service, then an error is returned.
pub fn set_syslog_servers(servers: Vec<String>) -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Vec<String>>(Node::Syslog(SubCommand::Set), servers) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Initiates syslog servers.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to open or write `/etc/rsyslog.d/50-default.conf`, then
///   an error is returned.
/// * If it fails to restart rsyslogd service, then an error is returned.
pub fn init_syslog_servers() -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Syslog(SubCommand::Init), None) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// (Re)start syslog services.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to restart rsyslogd service, then an error is returned.
pub fn start_syslog_servers() -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Syslog(SubCommand::Enable), None) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Returns the list of interface names.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
pub fn list_of_interfaces(prefix: Option<String>) -> Result<Vec<String>> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Interface(SubCommand::List), prefix) {
        run_roxy::<Vec<String>>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Returns the settings of interface. All interfafces if None for device name
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
pub fn interfaces(dev: Option<String>) -> Result<Option<Vec<(String, NicOutput)>>> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Interface(SubCommand::Get), dev) {
        run_roxy::<Option<Vec<(String, NicOutput)>>>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Sets an interface setting.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to read or write a netplan yaml conf file, then an error
///   is returned.
/// * If dhcp4 and static ip address or nameserver address is set in the same
///   interface, then an error is returned.
/// * If a user tries to set a new gateway address when another interface has
///   the same, then an error is returned.
pub fn set_interface(
    dev: String,
    addresses: Option<Vec<String>>,
    dhcp4: Option<bool>,
    gateway4: Option<String>,
    nameservers: Option<Vec<String>>,
) -> Result<String> {
    let nic = NicOutput::new(addresses, dhcp4, gateway4, nameservers);
    if let Ok(req) =
        NodeRequest::new::<(String, NicOutput)>(Node::Interface(SubCommand::Set), (dev, nic))
    {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Init the settings of an interface.
///
/// # Errors
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If the specified interface name is not found, then an error is returned.
/// * If it failed to load /etc/netplan yaml files, then an error is returned.
/// * If if failed to execute netplan apply command, then an error is returned.
/// * If it failed to execute ifconfig command, then an error is returned.
pub fn init_interface(dev: String) -> Result<String> {
    if let Ok(req) =
        NodeRequest::new::<Option<String>>(Node::Interface(SubCommand::Init), Some(dev))
    {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Removes interface/gateway/nameserver address or dhcp4 option of interface.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If it fails to read or write a netplan yaml conf file, then an error
///   is returned.
pub fn remove_interface(
    dev: String,
    addresses: Option<Vec<String>>,
    dhcp4: Option<bool>,
    gateway4: Option<String>,
    nameservers: Option<Vec<String>>,
) -> Result<String> {
    let nic = NicOutput::new(addresses, dhcp4, gateway4, nameservers);
    if let Ok(req) =
        NodeRequest::new::<(String, NicOutput)>(Node::Interface(SubCommand::Delete), (dev, nic))
    {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Reboots the system forcefully and immediately.
///
/// This function uses a direct system call (`nix::sys::reboot::reboot`) which does not
/// send termination signals to running processes. For a graceful shutdown that allows
/// processes to terminate cleanly, use `graceful_reboot()`.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If `nix::sys::reboot::reboot` fails, then an error is returned.
pub fn reboot() -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Reboot, None) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Turns the system off forcefully and immediately.
///
/// This function uses a direct system call (`nix::sys::reboot::reboot`) which does not
/// send termination signals to running processes. For a graceful shutdown that allows
/// processes to terminate cleanly, use `graceful_power_off()`.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If `nix::sys::reboot::reboot` fails, then an error is returned.
pub fn power_off() -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::PowerOff, None) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Reboots the system gracefully.
///
/// This function executes the system's `reboot` command, allowing services
/// and processes to terminate cleanly.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If executing the `reboot` command fails, then an error is returned.
pub fn graceful_reboot() -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::GracefulReboot, None) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Turns the system off gracefully.
///
/// This function executes the system's `poweroff` command, allowing services
/// and processes to terminate cleanly.
///
/// # Errors
///
/// The following errors are possible:
///
/// * If serialization of command arguments does not succeed, then an error
///   is returned.
/// * If spawning the roxy executable fails, then an error is returned.
/// * If delivering a command to roxy fails, then an error is returned.
/// * If a response message from roxy is invalid regarding JSON syntax or
///   is not successfully base64-decoded, then an error is returned.
/// * If executing the `poweroff` command fails, then an error is returned.
pub fn graceful_power_off() -> Result<String> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::GracefulPowerOff, None) {
        run_roxy::<String>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Return configured sshd port number.
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn get_sshd() -> Result<u16> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Sshd(SubCommand::Get), None) {
        run_roxy::<u16>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Restart sshd service.
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn start_sshd() -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Sshd(SubCommand::Enable), None) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Return configured NTP server FQDNs
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn get_ntp() -> Result<Option<Vec<String>>> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Ntp(SubCommand::Get), None) {
        run_roxy::<Option<Vec<String>>>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Set ntp servers
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn set_ntp(servers: Vec<String>) -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<Vec<String>>(Node::Ntp(SubCommand::Get), servers) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// (Re)Start ntp service
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn start_ntp() -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Ntp(SubCommand::Enable), None) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Stop ntp service
///
/// # Errors
///
/// * Return error if it fails to build request message
/// * Return error if `run_roxy` function returns error
pub fn stop_ntp() -> Result<bool> {
    if let Ok(req) = NodeRequest::new::<Option<String>>(Node::Ntp(SubCommand::Disable), None) {
        run_roxy::<bool>(req)
    } else {
        Err(anyhow!(FAIL_REQUEST))
    }
}

/// Response message from Roxy to caller
#[derive(Deserialize, Debug)]
pub enum TaskResult {
    Ok(String),
    Err(String),
}

// TODO: fix the exact path to "roxy"
//
/// # Errors
///
/// * Failure to spawn roxy
/// * Failure to write command to roxy
/// * Invalid json syntax in response message
/// * base64 decode error for reponse message
/// * Received execution error from roxy
///
/// # Panics
///
/// * panic if it failed to convert request message to json
pub fn run_roxy<T>(req: NodeRequest) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let mut child = Command::new("roxy")
        .env("PATH", "/opt/clumit/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(child_stdin) = child.stdin.take() {
        std::thread::spawn(move || {
            serde_json::to_writer(child_stdin, &req).expect("`Task` should serialize to JSON");
        });
    } else {
        return Err(anyhow!("failed to execute roxy"));
    }

    let output = child.wait_with_output()?;
    match serde_json::from_reader::<&[u8], TaskResult>(&output.stdout) {
        Ok(TaskResult::Ok(x)) => {
            let decoded = BASE64
                .decode(x.as_bytes())
                .map_err(|_| anyhow!("fail to decode response."))?;
            let (decoded, _) =
                bincode::serde::decode_from_slice(&decoded, bincode::config::legacy())?;
            Ok(decoded)
        }
        Ok(TaskResult::Err(x)) => Err(anyhow!("{}", x)),
        Err(e) => Err(anyhow!("fail to parse response. {}", e)),
    }
}
