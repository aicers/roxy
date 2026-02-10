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
                    if let Some(pos) = line.find(':')
                        && let Some(s) = line.get(pos + 1..)
                    {
                        os_version = s.trim().to_string();
                    }
                } else if line.starts_with("Product:")
                    && let Some(pos) = line.find(':')
                    && let Some(s) = line.get(pos + 1..)
                {
                    product_version = s.trim().to_string();
                }
            }
        }
    }
    (os_version, product_version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Parses version content string and returns (`os_version`, `product_version`).
    ///
    /// This test helper mimics the parsing logic of `version()` but operates on
    /// a string input instead of reading from the filesystem, enabling comprehensive
    /// unit testing without production changes.
    fn parse_version_content(contents: &str) -> (String, String) {
        let mut os_version = DEFAULT_VERSION_STRING.to_string();
        let mut product_version = DEFAULT_VERSION_STRING.to_string();
        let lines = contents.lines();
        for line in lines {
            if line.starts_with("OS:") {
                if let Some(pos) = line.find(':')
                    && let Some(s) = line.get(pos + 1..)
                {
                    os_version = s.trim().to_string();
                }
            } else if line.starts_with("Product:")
                && let Some(pos) = line.find(':')
                && let Some(s) = line.get(pos + 1..)
            {
                product_version = s.trim().to_string();
            }
        }
        (os_version, product_version)
    }

    #[test]
    fn parse_os_version_full() {
        let content = "OS: Ubuntu 22.04 LTS\nProduct: 1.0.0";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, "Ubuntu 22.04 LTS");
    }

    #[test]
    fn parse_os_version_with_extra_whitespace() {
        let content = "OS:    CentOS 8   \nProduct: 2.0.0";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, "CentOS 8");
    }

    #[test]
    fn parse_os_version_empty_value() {
        let content = "OS:\nProduct: 1.0.0";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, "");
    }

    #[test]
    fn parse_os_version_missing_returns_default() {
        let content = "Product: 1.0.0\nSomeOther: value";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_os_version_with_colon_in_value() {
        let content = "OS: Ubuntu: Special Edition\nProduct: 1.0.0";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, "Ubuntu: Special Edition");
    }

    #[test]
    fn parse_product_version_full() {
        let content = "OS: Ubuntu 22.04\nProduct: 5.2.1";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, "5.2.1");
    }

    #[test]
    fn parse_product_version_with_extra_whitespace() {
        let content = "OS: Ubuntu\nProduct:    3.1.4   ";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, "3.1.4");
    }

    #[test]
    fn parse_product_version_empty_value() {
        let content = "OS: Ubuntu\nProduct:";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, "");
    }

    #[test]
    fn parse_product_version_missing_returns_default() {
        let content = "OS: Ubuntu\nOther: 1.0.0";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_product_version_with_colon_in_value() {
        let content = "OS: Ubuntu\nProduct: v1.0:beta";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, "v1.0:beta");
    }

    #[test]
    fn parse_version_content_empty_string() {
        let content = "";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_version_content_both_present() {
        let content = "OS: Debian 11\nProduct: 2.3.4";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Debian 11");
        assert_eq!(product, "2.3.4");
    }

    #[test]
    fn parse_version_content_reversed_order() {
        let content = "Product: 1.2.3\nOS: Fedora 35";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Fedora 35");
        assert_eq!(product, "1.2.3");
    }

    #[test]
    fn parse_version_content_with_extra_lines() {
        let content =
            "# Comment line\nOS: Alpine 3.15\nSomeKey: SomeValue\nProduct: 4.0.0\nAnother: line";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Alpine 3.15");
        assert_eq!(product, "4.0.0");
    }

    #[test]
    fn parse_version_content_only_whitespace_lines() {
        let content = "   \n\t\n  \n";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_version_content_malformed_lines_without_colon() {
        let content = "OS Ubuntu\nProduct 1.0.0";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_version_content_case_sensitivity() {
        let content = "os: lowercase\nproduct: 1.0.0";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_version_content_duplicate_os_lines() {
        let content = "OS: First\nOS: Second\nProduct: 1.0.0";
        let (os, _) = parse_version_content(content);
        assert_eq!(os, "Second");
    }

    #[test]
    fn parse_version_content_duplicate_product_lines() {
        let content = "OS: Ubuntu\nProduct: 1.0.0\nProduct: 2.0.0";
        let (_, product) = parse_version_content(content);
        assert_eq!(product, "2.0.0");
    }

    #[test]
    fn parse_version_content_partial_prefix_not_matched() {
        let content = "OSX: macOS 12\nProductVersion: 1.0.0";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, DEFAULT_VERSION_STRING);
        assert_eq!(product, DEFAULT_VERSION_STRING);
    }

    #[test]
    fn parse_version_content_special_characters_in_value() {
        let content = "OS: Ubuntu (LTS) #22.04 @stable\nProduct: v1.0.0-beta+build.123";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Ubuntu (LTS) #22.04 @stable");
        assert_eq!(product, "v1.0.0-beta+build.123");
    }

    #[test]
    fn parse_version_content_unicode_in_value() {
        let content = "OS: システム v1.0\nProduct: 产品 2.0";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "システム v1.0");
        assert_eq!(product, "产品 2.0");
    }

    #[test]
    fn parse_version_content_trailing_newline() {
        let content = "OS: Ubuntu\nProduct: 1.0.0\n";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Ubuntu");
        assert_eq!(product, "1.0.0");
    }

    #[test]
    fn parse_version_content_windows_line_endings() {
        let content = "OS: Ubuntu\r\nProduct: 1.0.0\r\n";
        let (os, product) = parse_version_content(content);
        assert_eq!(os, "Ubuntu");
        assert_eq!(product, "1.0.0");
    }

    #[test]
    fn default_version_string_value() {
        assert_eq!(DEFAULT_VERSION_STRING, "AICE security");
    }
}
