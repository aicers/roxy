use super::run_command_output;
use std::{fmt::Write as FmtWrite, fs::File, io::Read};

const DEFAULT_VERSION_STRING: &str = "AICE security";
// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

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
        write!(status, " (boot: {output})").expect("writing to string should not fail");
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
