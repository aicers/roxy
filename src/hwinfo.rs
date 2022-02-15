use crate::{run_command_output, task::SubCommand};
use anyhow::{anyhow, Result};
use regex::Regex;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
};

const DATA_PARTITION: &str = "/data";
const DEFAULT_VERSION_STRING: &str = "AICE security";
// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

/// Get the usage of the partition mounted on `/data` using command `df -h`:
/// # Return
///   Mount point, Total size, Used size, Used rate
///
/// # Errors
/// * fail to compile regex
pub fn diskusage() -> Result<Option<(String, String, String, String)>> {
    if let Some(output) = run_command_output("df", None, &["-h"]) {
        let re = Regex::new(
            r#"(?P<f>[/a-z0-9]+)\s+(?P<s>[0-9\.]+[A-Za-z]+)\s+(?P<u>[0-9\.]+[A-Za-z]*)\s+(?P<a>[0-9\.]+[A-Za-z]*)\s+(?P<e>[0-9]+%)\s+(?P<m>[/a-z0-9]+)"#,
        )?;
        let lines = output.lines();
        for line in lines {
            if line.starts_with("/dev/") && line.ends_with(DATA_PARTITION) {
                let after = re.replace_all(line, "$m,$s,$u,$e");
                let v = after.as_ref().split(',').collect::<Vec<_>>();
                if let Some(mount) = v.get(0) {
                    if let Some(size) = v.get(1) {
                        if let Some(used) = v.get(2) {
                            if let Some(rate) = v.get(3) {
                                return Ok(Some((
                                    (*mount).to_string(),
                                    (*size).to_string(),
                                    (*used).to_string(),
                                    (*rate).to_string(),
                                )));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

/// Get system uptime. It's joined result for command `uptime -p` and `uptime -s`
/// # Example
/// ```
/// // Result format:
/// // how long the system has been running (boot: boot up time)
/// // up 7 weeks, 5 days, 13 hours, 52 minutes (boot: 2021-12-16 23:43:10)
/// if let Some(uptime) = hwinfo::uptime() {
///     println!("uptime = {}", uptime);
/// }
/// ```
#[must_use]
pub fn uptime() -> Option<String> {
    let mut status = String::new();
    if let Some(mut output) = run_command_output("uptime", None, &["-p"]) {
        output.pop();
        status.push_str(&output);
    }

    if let Some(mut output) = run_command_output("uptime", None, &["-s"]) {
        output.pop();
        status.push_str(&format!(" (boot: {})", output));
    }
    if status.is_empty() {
        None
    } else {
        Some(status)
    }
}

/// Get OS and Product versions. Refer /etc/version
/// # Example
/// ```
/// let (os_ver, product_ver) = hwinfo::get_version();
/// println!("OS version = {}, Product version = {}", os_ver, product_ver);
/// ```
#[must_use]
pub fn get_version() -> (String, String) {
    let mut os_version = DEFAULT_VERSION_STRING.to_string();
    let mut product_version = DEFAULT_VERSION_STRING.to_string();
    if let Ok(mut file) = File::open(DEFAULT_VERSION_PATH) {
        let mut contents = String::new();
        if file.read_to_string(&mut contents).is_ok() {
            let lines = contents.lines();
            for line in lines {
                if line.starts_with("OS:") {
                    if let Some(pos) = line.find(':') {
                        if let Some(s) = line.get(pos + 1..) {
                            os_version = s.trim().to_string();
                        }
                    }
                } else if line.starts_with("Product:") {
                    if let Some(pos) = line.find(':') {
                        if let Some(s) = line.get(pos + 1..) {
                            product_version = s.trim().to_string();
                        }
                    }
                }
            }
        }
    }
    (os_version, product_version)
}

/// Set OS or Product version. Refer /etc/version
/// # Example
/// ```
/// hwinfo::set_version(SubCommand::SetOsVersion, "AICE OS v1.0.23").unwrap();
/// hwinfo::set_version(SubCommand::SetProductVersion, "AICE Security v1.1.99").unwrap();
/// ```
/// # Errors
/// * fail to open or write /etc/version
pub fn set_version(kind: SubCommand, arg: &str) -> Result<()> {
    let contents = fs::read_to_string(DEFAULT_VERSION_PATH)?;
    let lines = contents.lines();
    let mut new_contents = String::new();
    for line in lines {
        match kind {
            SubCommand::SetOsVersion => {
                if line.to_lowercase().starts_with("os:") {
                    continue;
                }
            }
            SubCommand::SetProductVersion => {
                if line.to_lowercase().starts_with("product:") {
                    continue;
                }
            }
            _ => return Err(anyhow!("invalid command")),
        }

        new_contents.push_str(line);
        new_contents.push('\n');
    }

    let new_version = match kind {
        SubCommand::SetOsVersion => format!("OS: {}", arg),
        SubCommand::SetProductVersion => format!("Product: {}", arg),
        _ => return Err(anyhow!("invalid command")),
    };

    new_contents.push_str(&new_version);
    new_contents.push('\n');

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(DEFAULT_VERSION_PATH)?;

    file.write_all(new_contents.as_bytes())?;
    Ok(())
}
