use super::run_command_output;
use anyhow::Result;
use regex::Regex;
use std::{fmt::Write as FmtWrite, fs::File, io::Read};

const DATA_PARTITION: &str = "/data";
const DEFAULT_VERSION_STRING: &str = "AICE security";
// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

/// Returns usage of the partition mounted on `/data` using command `df -h`
/// as a tuple of mount point, total size, used size, and used rate.
///
/// # Errors
///
/// If `Regex` fails to compile a given regular expression,
/// then an error is returned.
pub fn disk_usage() -> Result<Option<(String, String, String, String)>> {
    if let Some(output) = run_command_output("df", None, &["-h"]) {
        let re = Regex::new(
            r#"(?P<f>[/a-z0-9]+)\s+(?P<s>[0-9\.]+[A-Za-z]+)\s+(?P<u>[0-9\.]+[A-Za-z]*)\s+(?P<a>[0-9\.]+[A-Za-z]*)\s+(?P<e>[0-9]+%)\s+(?P<m>[/a-z0-9]+)"#,
        )?;
        let lines = output.lines();
        for line in lines {
            if line.starts_with("/dev/") && line.ends_with(DATA_PARTITION) {
                let after = re.replace_all(line, "$m,$s,$u,$e");
                let mut values = after.as_ref().split(',');
                if let Some(mount) = values.next() {
                    if let Some(size) = values.next() {
                        if let Some(used) = values.next() {
                            if let Some(rate) = values.next() {
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

/// Returns how long the system has been running by `uptime -p` and `uptime -s`
///
/// # Example
///
/// ```ignore
/// // Result format:
/// // how long the system has been running (boot: boot up time)
/// // up 7 weeks, 5 days, 13 hours, 52 minutes (boot: 2021-12-16 23:43:10)
/// if let Some(uptime) = uptime() {
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
        write!(status, " (boot: {})", output).expect("writing to string should not fail");
    }
    if status.is_empty() {
        None
    } else {
        Some(status)
    }
}

/// Returns OS and Product versions by reading /etc/version.
///
/// # Example
///
/// ```ignore
/// let (os_ver, product_ver) = version();
/// println!("OS version = {}, Product version = {}", os_ver, product_ver);
/// ```
#[must_use]
pub fn version() -> (String, String) {
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
