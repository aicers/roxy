pub mod common;
mod user;

#[cfg(not(test))]
use std::process::{Command, Stdio};

use anyhow::{Result, anyhow};
pub use common::waitfor_up;
use common::{NicOutput, Node, NodeRequest, SubCommand};
#[cfg(not(test))]
use data_encoding::BASE64;
use serde::Deserialize;
pub use user::hwinfo::{uptime, version};
pub use user::process::{Process, process_list};
pub use user::usg::{ResourceUsage, resource_usage};
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
    if let Ok(req) = NodeRequest::new::<Vec<String>>(Node::Ntp(SubCommand::Set), servers) {
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
#[cfg(not(test))]
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
            Ok(bincode::deserialize::<T>(&decoded)?)
        }
        Ok(TaskResult::Err(x)) => Err(anyhow!("{x}")),
        Err(e) => Err(anyhow!("fail to parse response. {e}")),
    }
}

#[cfg(test)]
#[allow(clippy::missing_errors_doc)]
pub fn run_roxy<T>(req: NodeRequest) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    test_support::handle_request(req)
}

#[cfg(test)]
mod test_support {
    use std::sync::{Mutex, OnceLock, PoisonError};

    use serde::Serialize;

    use super::*;

    enum StubResponse {
        Ok(Vec<u8>),
        Err(String),
    }

    struct StubState {
        last_request: Option<NodeRequest>,
        next_response: Option<StubResponse>,
    }

    fn state() -> &'static Mutex<StubState> {
        static STATE: OnceLock<Mutex<StubState>> = OnceLock::new();
        STATE.get_or_init(|| {
            Mutex::new(StubState {
                last_request: None,
                next_response: None,
            })
        })
    }

    pub fn lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    pub fn reset() {
        let mut state = state().lock().expect("stub state");
        state.last_request = None;
        state.next_response = None;
    }

    pub fn set_ok_response<T: Serialize>(value: &T) {
        let bytes = bincode::serialize(value).expect("serialize test response");
        let mut state = state().lock().expect("stub state");
        state.next_response = Some(StubResponse::Ok(bytes));
    }

    pub fn set_err_response(message: &str) {
        let mut state = state().lock().expect("stub state");
        state.next_response = Some(StubResponse::Err(message.to_string()));
    }

    pub fn take_last_request() -> Option<NodeRequest> {
        let mut state = state().lock().expect("stub state");
        state.last_request.take()
    }

    pub fn handle_request<T: serde::de::DeserializeOwned>(req: NodeRequest) -> Result<T> {
        let mut state = state().lock().expect("stub state");
        state.last_request = Some(req);
        match state.next_response.take() {
            Some(StubResponse::Ok(bytes)) => {
                bincode::deserialize::<T>(&bytes).map_err(|e| anyhow!("{e}"))
            }
            Some(StubResponse::Err(message)) => Err(anyhow!("{message}")),
            None => Err(anyhow!("missing test response")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::MutexGuard;

    use serde::Serialize;

    use super::test_support;
    use super::*;

    struct TestCtx {
        guard: MutexGuard<'static, ()>,
    }

    impl TestCtx {
        fn new() -> Self {
            let guard = test_support::lock();
            test_support::reset();
            Self { guard }
        }

        fn request(self) -> NodeRequest {
            let _guard = self.guard;
            test_support::take_last_request().expect("request should be captured")
        }
    }

    fn set_ok<T: Serialize>(value: &T) {
        test_support::set_ok_response(value);
    }

    fn set_err(message: &str) {
        test_support::set_err_response(message);
    }

    fn assert_none_arg(req: &NodeRequest) {
        let decoded: Option<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, None);
    }

    #[test]
    fn test_hostname_returns_non_empty() {
        let hostname = hostname();
        assert!(!hostname.is_empty());
    }

    #[test]
    fn test_service_control_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&true);

        let result = service_control(SubCommand::Status, "nginx".to_string())
            .expect("service_control should succeed");
        assert!(result);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Service(SubCommand::Status));
        let decoded: String = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, "nginx");
    }

    #[test]
    fn test_set_os_version_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = set_os_version("24.04".to_string()).expect("set_os_version should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Version(SubCommand::SetOsVersion));
        let decoded: String = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, "24.04");
    }

    #[test]
    fn test_set_product_version_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result =
            set_product_version("1.2.3".to_string()).expect("set_product_version should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Version(SubCommand::SetProductVersion));
        let decoded: String = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, "1.2.3");
    }

    #[test]
    fn test_set_hostname_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = set_hostname("roxy-node".to_string()).expect("set_hostname should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Hostname(SubCommand::Set));
        let decoded: String = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, "roxy-node");
    }

    #[test]
    fn test_syslog_servers_builds_request_with_none() {
        let ctx = TestCtx::new();
        let expected = Some(vec![(
            "auth".to_string(),
            "tcp".to_string(),
            "127.0.0.1:514".to_string(),
        )]);
        set_ok(&expected);

        let result = syslog_servers().expect("syslog_servers should succeed");
        assert_eq!(result, expected);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Syslog(SubCommand::Get));
        assert_none_arg(&req);
    }

    #[test]
    fn test_set_syslog_servers_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());
        let servers = vec!["tcp://127.0.0.1:514".to_string()];

        let result =
            set_syslog_servers(servers.clone()).expect("set_syslog_servers should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Syslog(SubCommand::Set));
        let decoded: Vec<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, servers);
    }

    #[test]
    fn test_init_syslog_servers_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = init_syslog_servers().expect("init_syslog_servers should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Syslog(SubCommand::Init));
        assert_none_arg(&req);
    }

    #[test]
    fn test_start_syslog_servers_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&true);

        let result = start_syslog_servers().expect("start_syslog_servers should succeed");
        assert!(result);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Syslog(SubCommand::Enable));
        assert_none_arg(&req);
    }

    #[test]
    fn test_list_of_interfaces_builds_request() {
        let ctx = TestCtx::new();
        let expected = vec!["eth0".to_string(), "eth1".to_string()];
        set_ok(&expected);

        let result =
            list_of_interfaces(Some("eth".to_string())).expect("list_of_interfaces should succeed");
        assert_eq!(result, expected);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Interface(SubCommand::List));
        let decoded: Option<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, Some("eth".to_string()));
    }

    #[test]
    fn test_interfaces_builds_request() {
        let ctx = TestCtx::new();
        let expected = Some(vec![(
            "eth0".to_string(),
            NicOutput::new(
                Some(vec!["192.168.0.2/24".to_string()]),
                Some(false),
                Some("192.168.0.1".to_string()),
                Some(vec!["8.8.8.8".to_string()]),
            ),
        )]);
        set_ok(&expected);

        let result = interfaces(Some("eth0".to_string())).expect("interfaces should succeed");
        let entries = result.expect("interfaces response should be Some");
        assert_eq!(entries.len(), 1);
        let (name, nic) = &entries[0];
        assert_eq!(name, "eth0");
        assert_eq!(nic.addresses, Some(vec!["192.168.0.2/24".to_string()]));
        assert_eq!(nic.dhcp4, Some(false));
        assert_eq!(nic.gateway4, Some("192.168.0.1".to_string()));
        assert_eq!(nic.nameservers, Some(vec!["8.8.8.8".to_string()]));

        let req = ctx.request();
        assert_eq!(req.kind, Node::Interface(SubCommand::Get));
        let decoded: Option<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, Some("eth0".to_string()));
    }

    #[test]
    fn test_set_interface_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = set_interface(
            "eth0".to_string(),
            Some(vec!["10.0.0.2/24".to_string()]),
            Some(false),
            Some("10.0.0.1".to_string()),
            Some(vec!["1.1.1.1".to_string()]),
        )
        .expect("set_interface should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Interface(SubCommand::Set));
        let decoded: (String, NicOutput) =
            bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded.0, "eth0");
        assert_eq!(decoded.1.addresses, Some(vec!["10.0.0.2/24".to_string()]));
        assert_eq!(decoded.1.dhcp4, Some(false));
        assert_eq!(decoded.1.gateway4, Some("10.0.0.1".to_string()));
        assert_eq!(decoded.1.nameservers, Some(vec!["1.1.1.1".to_string()]));
    }

    #[test]
    fn test_init_interface_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = init_interface("eth1".to_string()).expect("init_interface should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Interface(SubCommand::Init));
        let decoded: Option<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, Some("eth1".to_string()));
    }

    #[test]
    fn test_remove_interface_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = remove_interface(
            "eth0".to_string(),
            None,
            Some(false),
            Some("10.0.0.1".to_string()),
            Some(vec!["9.9.9.9".to_string()]),
        )
        .expect("remove_interface should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Interface(SubCommand::Delete));
        let decoded: (String, NicOutput) =
            bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded.0, "eth0");
        assert_eq!(decoded.1.addresses, None);
        assert_eq!(decoded.1.dhcp4, Some(false));
        assert_eq!(decoded.1.gateway4, Some("10.0.0.1".to_string()));
        assert_eq!(decoded.1.nameservers, Some(vec!["9.9.9.9".to_string()]));
    }

    #[test]
    fn test_reboot_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = reboot().expect("reboot should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Reboot);
        assert_none_arg(&req);
    }

    #[test]
    fn test_power_off_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = power_off().expect("power_off should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::PowerOff);
        assert_none_arg(&req);
    }

    #[test]
    fn test_graceful_reboot_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = graceful_reboot().expect("graceful_reboot should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::GracefulReboot);
        assert_none_arg(&req);
    }

    #[test]
    fn test_graceful_power_off_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&"ok".to_string());

        let result = graceful_power_off().expect("graceful_power_off should succeed");
        assert_eq!(result, "ok");

        let req = ctx.request();
        assert_eq!(req.kind, Node::GracefulPowerOff);
        assert_none_arg(&req);
    }

    #[test]
    fn test_get_sshd_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&2222_u16);

        let result = get_sshd().expect("get_sshd should succeed");
        assert_eq!(result, 2222);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Sshd(SubCommand::Get));
        assert_none_arg(&req);
    }

    #[test]
    fn test_get_ntp_builds_request() {
        let ctx = TestCtx::new();
        let expected = Some(vec!["time.example.org".to_string()]);
        set_ok(&expected);

        let result = get_ntp().expect("get_ntp should succeed");
        assert_eq!(result, expected);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Ntp(SubCommand::Get));
        assert_none_arg(&req);
    }

    #[test]
    fn test_set_ntp_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&true);
        let servers = vec![
            "time.example.org".to_string(),
            "time2.example.org".to_string(),
        ];

        let result = set_ntp(servers.clone()).expect("set_ntp should succeed");
        assert!(result);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Ntp(SubCommand::Set));
        let decoded: Vec<String> = bincode::deserialize(&req.arg).expect("arg should decode");
        assert_eq!(decoded, servers);
    }

    #[test]
    fn test_start_ntp_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&true);

        let result = start_ntp().expect("start_ntp should succeed");
        assert!(result);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Ntp(SubCommand::Enable));
        assert_none_arg(&req);
    }

    #[test]
    fn test_stop_ntp_builds_request() {
        let ctx = TestCtx::new();
        set_ok(&true);

        let result = stop_ntp().expect("stop_ntp should succeed");
        assert!(result);

        let req = ctx.request();
        assert_eq!(req.kind, Node::Ntp(SubCommand::Disable));
        assert_none_arg(&req);
    }

    #[test]
    fn test_start_sshd_propagates_roxy_error() {
        let ctx = TestCtx::new();
        set_err("roxy failed");

        let err = start_sshd().expect_err("start_sshd should fail");
        assert_eq!(err.to_string(), "roxy failed");

        let req = ctx.request();
        assert_eq!(req.kind, Node::Sshd(SubCommand::Enable));
        assert_none_arg(&req);
    }
}
