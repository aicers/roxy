use std::{
    fmt::Write as FmtWrite,
    fs::{self, OpenOptions},
    io::Write as IoWrite,
};

use anyhow::Result;
use regex::Regex;

const NTP_CONF: &str = "/etc/ntp.conf";
const NTP_SERVICE_UNIT: &str = "ntp";

// Set NTP server addresses.
//
// # Example
//
// let ret = ntp::set(&vec!["time.bora.net".to_string(), "time2.kriss.re.kr".to_string()])?;
//
// # Errors
//
// * fail to open /etc/ntp.conf
// * fail to write modified contents to /etc/ntp.conf
// * fail to restart ntp service
pub(crate) fn set(servers: &[String]) -> Result<bool> {
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
        writeln!(new_contents, "server {server} iburst")
            .expect("writing to string should not fail");
    }

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(NTP_CONF)?;

    file.write_all(new_contents.as_bytes())?;

    let systemctl = systemctl::SystemCtl::default();
    systemctl
        .restart(NTP_SERVICE_UNIT)
        .map(|status| status.success())
        .map_err(Into::into)
}

// Get ntp server addresses.
//
// # Errors
//
// * fail to open /etc/ntp.conf
pub(crate) fn get() -> Result<Option<Vec<String>>> {
    let re = Regex::new(r"server\s+([a-z0-9\.]+)\s+iburst")?;
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

// True if ntp service is active
#[must_use]
pub(crate) fn is_active() -> bool {
    let systemctl = systemctl::SystemCtl::default();
    if let Ok(true) = systemctl.exists(NTP_SERVICE_UNIT) {
        systemctl.is_active(NTP_SERVICE_UNIT).is_ok_and(|ret| ret)
    } else {
        false
    }
}

// Start ntp client service
//
// # Errors
//
// * systemctl return error when starting ntp service
pub(crate) fn enable() -> Result<bool> {
    let systemctl = systemctl::SystemCtl::default();
    if let Ok(true) = systemctl.exists(NTP_SERVICE_UNIT) {
        systemctl
            .restart(NTP_SERVICE_UNIT)
            .map(|status| status.success())
            .map_err(Into::into)
    } else {
        Ok(false)
    }
}

// Stop ntp client service
//
// # Errors
//
// * systemctl return error when stopping ntp service
pub(crate) fn disable() -> Result<bool> {
    let systemctl = systemctl::SystemCtl::default();
    if let Ok(true) = systemctl.exists(NTP_SERVICE_UNIT) {
        systemctl
            .stop(NTP_SERVICE_UNIT)
            .map(|status| status.success())
            .map_err(Into::into)
    } else {
        Ok(false)
    }
}
