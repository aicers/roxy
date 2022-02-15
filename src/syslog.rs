use crate::run_command;
use anyhow::{anyhow, Result};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::net::SocketAddr;

const RSYSLOG_CONF: &str = "/etc/rsyslog.d/50-default.conf";
const DEFAULT_FACILITY: &str = "user.*";

/// Set or init rsyslog remote servers. Currently the facility is fixed to `user.*`.
/// # Example
/// ```
/// // To set remote addresses:
/// let cmd = Some(vec![
///     "@@192.168.0.205:7500".to_string(), // tcp
///     "@192.168.1.71:500".to_string()     // udp
/// ]);
/// let ret = syslog::set(&cmd)?;
///
/// // To init(delete) remote addresses:
/// let ret = syslog::set(None)?;
/// ```
/// # Errors
/// * invalid protocol, remote address, port
/// * fail to open /etc/rsyslog.d/50-default.conf
/// * fail to write modified contents to /etc/rsyslog.d/50-default.conf
/// * fail to restart rsyslogd service
pub fn set(remote_addrs: &Option<Vec<String>>) -> Result<()> {
    if let Some(addrs) = remote_addrs {
        for addr in addrs {
            let _addr = addr
                .replace("@", "")
                .trim()
                .parse::<SocketAddr>()
                .map_err(|e| anyhow!("invalid address: {:?}", e))?;
        }
    }

    let contents = fs::read_to_string(RSYSLOG_CONF)?;
    let lines = contents.lines();
    let mut new_contents = String::new();
    for line in lines {
        if line.starts_with('#') || !line.contains('@') {
            new_contents.push_str(line);
            new_contents.push('\n');
        }
    }

    if let Some(addrs) = remote_addrs {
        for addr in addrs {
            new_contents.push_str(&format!("{} {}\n", DEFAULT_FACILITY, addr));
        }
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(RSYSLOG_CONF)?;

    file.write_all(new_contents.as_bytes())?;

    run_command("systemctl", None, &["restart", "rsyslog"])
}

/// Get rsyslog remote servers.
/// # Example
/// ```
/// if let Some(addrs) = syslog::get() {
///     for (facility, proto, addr) in &addrs {
///         println!("facility = {}, proto = {}, dest addr = {}", facility, proto, addr);
///     }
/// }
/// ```
/// # Errors
/// * fail to open /etc/rsyslog.d/50-default.conf
pub fn get() -> Result<Option<Vec<(String, String, String)>>> {
    let contents = fs::read_to_string(RSYSLOG_CONF)?;
    let lines = contents.lines();

    let mut ret = Vec::new();
    for line in lines {
        if line.starts_with('#') {
            continue;
        }

        let (r, proto) = if line.contains("@@") {
            (
                line.trim().split("@@").collect::<Vec<_>>(),
                "tcp".to_string(),
            )
        } else if line.contains('@') {
            (
                line.trim().split('@').collect::<Vec<_>>(),
                "udp".to_string(),
            )
        } else {
            continue;
        };

        if r.len() == 2 {
            if let Some(first) = r.first() {
                let facility = (*first).trim().to_string();
                if let Some(last) = r.last() {
                    if !last.trim().is_empty() {
                        ret.push((facility, proto, (*last).to_string()));
                    }
                }
            }
        }
    }

    if ret.is_empty() {
        Ok(None)
    } else {
        Ok(Some(ret))
    }
}
