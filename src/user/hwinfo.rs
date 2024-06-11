use std::{fmt, fs::File, io::Read, time::Duration};

use thiserror::Error;

const DEFAULT_VERSION_STRING: &str = "AICE security";
// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

#[derive(Debug, Error)]
pub struct UptimeError {
    message: String,
}

impl fmt::Display for UptimeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Returns how long the system has been running.
///
/// # Errors
///
/// Returns an error if the operating system does not return uptime or boottime.
///
/// # Examples
///
/// ```rust
/// # use std::error::Error;
/// # fn main() -> Result<(), Box<dyn Error>> {
/// let uptime = roxy::uptime()?;
/// #     Ok(())
/// # }
/// ```
pub fn uptime() -> Result<Duration, UptimeError> {
    uptime_lib::get().map_err(|e| UptimeError {
        message: e.to_string(),
    })
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
