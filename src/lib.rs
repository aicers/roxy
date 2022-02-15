pub mod hwinfo;
pub mod ifconfig;
pub mod ntp;
pub mod sshd;
pub mod syslog;
pub mod task;
pub mod ufw;

use anyhow::{anyhow, Result};
use chrono::{DateTime, Local};
use ipnet::IpNet;
use std::{fs, net::IpAddr, process::Command};

/// Validate ipv4/ipv6 networks
/// # Errors
/// * invalid ip network format
pub fn validate_ipnetworks(ipnetwork: &str) -> Result<()> {
    match ipnetwork.parse::<IpNet>() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{:?}", e)),
    }
}

/// Validate ipv4, ipv6 address
/// # Errors
/// * invalid ip address format
pub fn validate_ipaddress(ipaddr: &str) -> Result<()> {
    match ipaddr.parse::<IpAddr>() {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!("{:?}", e)),
    }
}

/// Get file list in the specified folder. No recursive into sub folder.
/// # Errors
/// * dir is not exist or fail to read dir
/// * fail to get metadata from file
/// * fail to get modified time from file
pub fn list_files(
    dir: &str,
    except: Option<&[&str]>,
    subdir: bool,
) -> Result<Vec<(u64, String, String)>> {
    let paths = fs::read_dir(dir)?;

    let mut files = Vec::new();
    for path in paths.flatten() {
        let filepath = path.path();
        let metadata = fs::metadata(&filepath)?;
        let modified: DateTime<Local> = metadata.modified()?.into();

        if let Some(filename) = path.path().file_name() {
            if let Some(filename) = filename.to_str() {
                if metadata.is_file() {
                    files.push((
                        metadata.len(),
                        format!("{}", modified.format("%Y/%m/%d %T")),
                        filename.to_string(),
                    ));
                } else if subdir && metadata.is_dir() {
                    files.push((0, String::new(), filename.to_string()));
                    /*
                    if let Ok(ret) = list_files(filename, except, subdir) {
                        for (size, modified_time, name) in ret {
                            files.push((size, modified_time, format!("{}/{}", filename, name)));
                        }
                    }
                    */
                }
            }
        }
    }
    if let Some(except) = except {
        for prefix in except {
            files.retain(|(_, _, name)| !name.starts_with(prefix));
        }
    }
    files.sort_by(|a, b| a.2.cmp(&b.2));
    Ok(files)
}

pub const DEFAULT_PATH_ENV: &str = "/usr/sbin:/usr/bin:/sbin:/bin:/usr/local/aice/bin";

/// Run linux command
/// # Errors
/// * get error code from executed command
pub fn run_command(cmd: &str, path: Option<&[&str]>, args: &[&str]) -> Result<()> {
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
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }

    match cmd.status() {
        Ok(status) => {
            let _r = status.success();
            Ok(())
        }
        Err(e) => Err(anyhow!("{}", e)),
    }
}

/// Run linux command and return it's output
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
