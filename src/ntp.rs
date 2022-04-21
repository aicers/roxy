use crate::{run_command, run_command_output};
use anyhow::Result;
use regex::Regex;
use std::fs::{self, OpenOptions};
use std::io::Write;

const NTP_CONF: &str = "/etc/ntp.conf";

/// Set NTP server addresses.
///
/// # Example
///
/// ```ignore
/// let ret = ntp::set(&vec!["time.bora.net".to_string(), "time2.kriss.re.kr".to_string()])?;
/// ```
///
/// # Errors
///
/// * fail to open /etc/ntp.conf
/// * fail to write modified contents to /etc/ntp.conf
/// * fail to restart ntp service
pub fn set(servers: &[String]) -> Result<bool> {
    let contents = fs::read_to_string(NTP_CONF)?;
    let lines = contents.lines();
    let mut new_contents = String::new();
    for line in lines {
        if !line.starts_with("server ") {
            new_contents.push_str(line);
            new_contents.push('\n');
        }
    }

    for server in servers {
        new_contents.push_str(&format!("server {} iburst\n", server));
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(NTP_CONF)?;

    file.write_all(new_contents.as_bytes())?;

    run_command("systemctl", None, &["restart", "ntp"])
}

/// Get ntp server addresses.
/// # Errors
/// * fail to open /etc/ntp.conf
pub fn get() -> Result<Option<Vec<String>>> {
    let re = Regex::new(r#"server\s+([a-z0-9\.]+)\s+iburst"#)?;
    let contents = fs::read_to_string(NTP_CONF)?;
    let lines = contents.lines();

    let mut ret = Vec::new();
    for line in lines {
        if line.starts_with("server ") {
            if let Some(cap) = re.captures(line) {
                if let Some(server) = cap.get(1) {
                    ret.push(server.as_str().to_string());
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

/// True if ntp is active
#[must_use]
pub fn is_active() -> bool {
    if let Some(output) = run_command_output("systemctl", None, &["is-active", "ntp"]) {
        output == "active"
    } else {
        false
    }
}

/// Enable ntp client service
/// # Errors
/// * fail to run systemctl start ntp command
pub fn enable() -> Result<bool> {
    run_command("systemctl", None, &["start", "ntp"])
}

/// Disable ntp client service
/// # Errors
/// * fail to run systemctl stop ntp command
pub fn disable() -> Result<bool> {
    run_command("systemctl", None, &["stop", "ntp"])
}
