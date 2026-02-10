use std::{
    fs::{self, OpenOptions},
    io::Write as IoWrite,
};

use anyhow::{Result, anyhow};

use super::SubCommand;

// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

/// Transforms the version file contents by updating the OS or Product version.
///
/// This function filters out existing lines matching the target key (case-insensitive)
/// and appends the new version line at the end.
///
/// # Returns
///
/// Returns `Ok(new_contents)` on success, or `Err` if the command is invalid.
///
/// # Errors
///
/// Returns an error if `kind` is not `SetOsVersion` or `SetProductVersion`.
fn transform_version_contents(contents: &str, kind: SubCommand, arg: &str) -> Result<String> {
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
        SubCommand::SetOsVersion => format!("OS: {arg}"),
        SubCommand::SetProductVersion => format!("Product: {arg}"),
        _ => return Err(anyhow!("invalid command")),
    };

    new_contents.push_str(&new_version);
    new_contents.push('\n');

    Ok(new_contents)
}

pub(crate) fn set_version(kind: SubCommand, arg: &str) -> Result<()> {
    let contents = fs::read_to_string(DEFAULT_VERSION_PATH)?;
    let new_contents = transform_version_contents(&contents, kind, arg)?;

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(DEFAULT_VERSION_PATH)?;

    file.write_all(new_contents.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test: OS version update changes the OS line when it differs from the existing one.
    #[test]
    fn os_update_changes_value_when_different() {
        let original = "OS: Ubuntu 20.04\nProduct: AICE 1.0\nCustom: data\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "Ubuntu 22.04")
            .expect("valid command should succeed");

        assert!(result.contains("OS: Ubuntu 22.04\n"));
        assert!(!result.contains("OS: Ubuntu 20.04"));
        assert!(result.contains("Product: AICE 1.0\n"));
        assert!(result.contains("Custom: data\n"));
    }

    // Test: OS version update is a no-op when the new value matches the existing one
    // (the line is still replaced, but content is equivalent).
    #[test]
    fn os_update_replaces_with_identical_value() {
        let original = "OS: Ubuntu 22.04\nProduct: AICE 1.0\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "Ubuntu 22.04")
            .expect("valid command should succeed");

        assert!(result.contains("OS: Ubuntu 22.04\n"));
        assert!(result.contains("Product: AICE 1.0\n"));
        // The OS line should appear exactly once (at the end after transformation).
        assert_eq!(result.matches("OS:").count(), 1);
    }

    // Test: Product version update changes the Product line when it differs.
    #[test]
    fn product_update_changes_value_when_different() {
        let original = "OS: Ubuntu 20.04\nProduct: AICE 1.0\nCustom: data\n";
        let result =
            transform_version_contents(original, SubCommand::SetProductVersion, "AICE 2.0")
                .expect("valid command should succeed");

        assert!(result.contains("Product: AICE 2.0\n"));
        assert!(!result.contains("Product: AICE 1.0"));
        assert!(result.contains("OS: Ubuntu 20.04\n"));
        assert!(result.contains("Custom: data\n"));
    }

    // Test: Product version update is a no-op when the new value matches the existing one.
    #[test]
    fn product_update_replaces_with_identical_value() {
        let original = "OS: Ubuntu 22.04\nProduct: AICE 1.0\n";
        let result =
            transform_version_contents(original, SubCommand::SetProductVersion, "AICE 1.0")
                .expect("valid command should succeed");

        assert!(result.contains("Product: AICE 1.0\n"));
        assert!(result.contains("OS: Ubuntu 22.04\n"));
        assert_eq!(result.matches("Product:").count(), 1);
    }

    // Test: Unrelated lines are preserved during OS version update.
    #[test]
    fn os_update_preserves_unrelated_lines() {
        let original = "Comment: some info\nOS: old\nExtra: value\nProduct: v1\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "new")
            .expect("valid command should succeed");

        assert!(result.contains("Comment: some info\n"));
        assert!(result.contains("Extra: value\n"));
        assert!(result.contains("Product: v1\n"));
        assert!(result.contains("OS: new\n"));
        assert!(!result.contains("OS: old"));
    }

    // Test: Unrelated lines are preserved during Product version update.
    #[test]
    fn product_update_preserves_unrelated_lines() {
        let original = "Comment: some info\nOS: v1\nExtra: value\nProduct: old\n";
        let result = transform_version_contents(original, SubCommand::SetProductVersion, "new")
            .expect("valid command should succeed");

        assert!(result.contains("Comment: some info\n"));
        assert!(result.contains("Extra: value\n"));
        assert!(result.contains("OS: v1\n"));
        assert!(result.contains("Product: new\n"));
        assert!(!result.contains("Product: old"));
    }

    // Test: Case-insensitive matching for OS line (e.g., "os:", "Os:", "OS:").
    #[test]
    fn os_update_matches_case_insensitively() {
        let original = "os: lowercase\nOs: mixed\nOS: uppercase\nOther: data\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "replaced")
            .expect("valid command should succeed");

        // All OS lines should be removed and replaced by one new line.
        assert_eq!(result.matches("OS:").count(), 1);
        assert!(!result.contains("os: lowercase"));
        assert!(!result.contains("Os: mixed"));
        assert!(result.contains("OS: replaced\n"));
        assert!(result.contains("Other: data\n"));
    }

    // Test: Case-insensitive matching for Product line.
    #[test]
    fn product_update_matches_case_insensitively() {
        let original = "product: lowercase\nProduct: mixed\nPRODUCT: uppercase\nOther: data\n";
        let result =
            transform_version_contents(original, SubCommand::SetProductVersion, "replaced")
                .expect("valid command should succeed");

        assert_eq!(result.matches("Product:").count(), 1);
        assert!(!result.contains("product: lowercase"));
        assert!(!result.contains("PRODUCT: uppercase"));
        assert!(result.contains("Product: replaced\n"));
        assert!(result.contains("Other: data\n"));
    }

    // Test: Empty file contents results in only the new version line.
    #[test]
    fn os_update_handles_empty_contents() {
        let result = transform_version_contents("", SubCommand::SetOsVersion, "Ubuntu 22.04")
            .expect("valid command should succeed");

        assert_eq!(result, "OS: Ubuntu 22.04\n");
    }

    // Test: Empty file contents for product version.
    #[test]
    fn product_update_handles_empty_contents() {
        let result = transform_version_contents("", SubCommand::SetProductVersion, "AICE 1.0")
            .expect("valid command should succeed");

        assert_eq!(result, "Product: AICE 1.0\n");
    }

    // Test: Empty argument string is accepted (results in "OS: " or "Product: ").
    #[test]
    fn os_update_accepts_empty_arg() {
        let original = "OS: old\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "")
            .expect("valid command should succeed");

        assert_eq!(result, "OS: \n");
    }

    // Test: Empty argument string for product version.
    #[test]
    fn product_update_accepts_empty_arg() {
        let original = "Product: old\n";
        let result = transform_version_contents(original, SubCommand::SetProductVersion, "")
            .expect("valid command should succeed");

        assert_eq!(result, "Product: \n");
    }

    // Test: File without OS line adds a new OS line.
    #[test]
    fn os_update_adds_line_when_missing() {
        let original = "Product: AICE 1.0\nCustom: data\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "Ubuntu 22.04")
            .expect("valid command should succeed");

        assert!(result.contains("OS: Ubuntu 22.04\n"));
        assert!(result.contains("Product: AICE 1.0\n"));
        assert!(result.contains("Custom: data\n"));
    }

    // Test: File without Product line adds a new Product line.
    #[test]
    fn product_update_adds_line_when_missing() {
        let original = "OS: Ubuntu 22.04\nCustom: data\n";
        let result =
            transform_version_contents(original, SubCommand::SetProductVersion, "AICE 1.0")
                .expect("valid command should succeed");

        assert!(result.contains("Product: AICE 1.0\n"));
        assert!(result.contains("OS: Ubuntu 22.04\n"));
        assert!(result.contains("Custom: data\n"));
    }

    // Test: Invalid SubCommand returns an error.
    #[test]
    fn invalid_subcommand_returns_error() {
        let original = "OS: Ubuntu 22.04\n";
        let result = transform_version_contents(original, SubCommand::Get, "test");

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid command"));
    }

    // Test: New version line is appended at the end of the file.
    #[test]
    fn version_line_appended_at_end() {
        let original = "First: line\nSecond: line\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "Ubuntu 22.04")
            .expect("valid command should succeed");

        assert!(result.ends_with("OS: Ubuntu 22.04\n"));
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.last(), Some(&"OS: Ubuntu 22.04"));
    }

    // Test: Lines starting with "os:" prefix but having extra characters are preserved
    // (e.g., "os_version:" should not match "os:").
    #[test]
    fn os_update_does_not_match_similar_prefixes() {
        let original = "os_version: 1.0\nOS: old\nosinfo: data\n";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "new")
            .expect("valid command should succeed");

        assert!(result.contains("os_version: 1.0\n"));
        assert!(result.contains("osinfo: data\n"));
        assert!(result.contains("OS: new\n"));
        assert!(!result.contains("OS: old"));
    }

    // Test: Lines starting with "product:" prefix but having extra characters are preserved.
    #[test]
    fn product_update_does_not_match_similar_prefixes() {
        let original = "product_name: test\nProduct: old\nproductinfo: data\n";
        let result = transform_version_contents(original, SubCommand::SetProductVersion, "new")
            .expect("valid command should succeed");

        assert!(result.contains("product_name: test\n"));
        assert!(result.contains("productinfo: data\n"));
        assert!(result.contains("Product: new\n"));
        assert!(!result.contains("Product: old"));
    }

    // Test: Contents without trailing newline are handled correctly.
    #[test]
    fn handles_contents_without_trailing_newline() {
        let original = "OS: old\nProduct: v1";
        let result = transform_version_contents(original, SubCommand::SetOsVersion, "new")
            .expect("valid command should succeed");

        assert!(result.contains("Product: v1\n"));
        assert!(result.contains("OS: new\n"));
        assert!(!result.contains("OS: old"));
    }

    // Note: The following branches cannot be tested without production changes:
    // - `set_version` I/O operations (reading/writing to /etc/version):
    //   The function uses a hardcoded path that requires root privileges.
    //   Testing would require either dependency injection for the path or
    //   mocking the filesystem, both of which are production changes.
}
