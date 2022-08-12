use anyhow::Result;
use regex::Regex;

use crate::{run_command, run_command_output};
pub type AccessLists = Vec<(String, String, String, Option<String>, Option<String>)>;

/// Get firewall rules. The result of command `ufw status`.
/// Currently only return IPv4 rules.
///
/// # Return
///
/// Vec<(Action, From, To, Protocol, Interface)>
/// * Action: String,        // ALLOW IN, ALLOW OUT, DENY IN, DENY OUT
/// * From: String,          // Source address, port (or "Any")
/// * To: String,            // Destination address, port (or "Any")
/// * Protocol: Option<String>  // Protocol name
/// * Interface: Option<String> // Interface name
/*
$  ufw status
Status: active

To                         Action      From
--                         ------      ----
22/tcp                     ALLOW IN    Anywhere
80/tcp                     ALLOW IN    Anywhere
443/tcp                    ALLOW IN    Anywhere
25/tcp                     DENY IN     Anywhere
25/tcp                     DENY OUT    Anywhere
22/tcp (v6)                ALLOW IN    Anywhere (v6)
80/tcp (v6)                ALLOW IN    Anywhere (v6)
443/tcp (v6)               ALLOW IN    Anywhere (v6)
25/tcp (v6)                DENY IN     Anywhere (v6)
25/tcp (v6)                DENY OUT    Anywhere (v6)
Anywhere                   DENY IN     203.0.113.100
Anywhere on eth0           ALLOW IN    203.0.113.102"#;
*/
/// # Errors
/// * fail to execute ufw status command
/// * fail to compile regex to parse ufw rules
pub fn get() -> Result<Option<AccessLists>> {
    if let Some(output) = run_command_output("ufw", None, &["status"]) {
        let re_action = Regex::new(r#"(?P<a>ALLOW|DENY)\s(?P<d>IN|OUT)"#)?;
        let re_dev = Regex::new(r#"(on\s[a-z0-9]+)"#)?;
        let re_proto = Regex::new(r#"(/[a-z]+)"#)?;
        let mut ret = Vec::new();
        let lines = output.lines();
        for line in lines {
            let mut after = line.replace("Anywhere", "Any");
            let proto = if let Some(cap) = re_proto.captures(&after) {
                if let Some(p) = cap.get(1) {
                    let p = p.as_str().to_string();
                    after = after.replace(&p, "");
                    Some(p.replace('/', ""))
                } else {
                    None
                }
            } else {
                None
            };
            let dev = if let Some(cap) = re_dev.captures(&after) {
                if let Some(dev) = cap.get(1) {
                    let dev_name = dev.as_str().to_string();
                    after = after.replace(&dev_name, "");
                    Some(dev_name.replace("on ", ""))
                } else {
                    None
                }
            } else {
                None
            };
            let after = re_action.replace_all(&after, ",$a $d,");
            let mut values = after.split(',');
            if let Some(to) = values.next() {
                if let Some(action) = values.next() {
                    if let Some(from) = values.next() {
                        ret.push((
                            action.trim().to_string(),
                            from.trim().to_string(),
                            to.trim().to_string(),
                            proto,
                            dev,
                        ));
                    }
                }
            }
        }

        if !ret.is_empty() {
            return Ok(Some(ret));
        }
    }
    Ok(None)
}

/// Add new rules.
/// This function execute `ufw add` command internally.
///
/// # Example
///
/// ```ignore
/// // UFW rule syntax
/// // allow|deny [in on <dev>] [from <src>] [to <dst>] [port <port>] [proto <protocol>]
/// let rules_to_add = vec![
///     "allow in on eth0 to any port 80 proto tcp".to_string(),
///     "allow in on eth0 from 203.0.113.102".to_string(),
///     "allow from 203.0.113.0/24 to any port 22 proto tcp".to_string()
/// ];
/// ufw::add(&rules_to_add).unwrap();
/// ```
/// # Errors
/// * fail to run ufw command
pub fn add(args: &[String]) -> Result<()> {
    for rule in args {
        run_command("ufw", None, &[rule.as_str()])?;
    }
    Ok(())
}

/// Remove rules.
/// This function execute `ufw delete` command internally.
///
/// # Example
///
/// ```ignore
/// let rules_to_delete = vec![
///     "allow from 203.0.113.101".to_string(),
///     "allow from 203.0.113.0/24 proto tcp to any port 22".to_string()];
/// ufw::delete(&rules_to_delete).unwrap();
/// ```
/// # Errors
/// * fail to run ufw delete command
pub fn delete(args: &[String]) -> Result<()> {
    for rule_id in args {
        run_command("ufw", None, &["delete", rule_id])?;
    }

    Ok(())
}

/// Enable ufw
/// # Errors
/// * fail to run ufw enable command
pub fn enable() -> Result<bool> {
    run_command("ufw", None, &["enable"])
}

/// Disable ufw
/// # Errors
/// * fail to run ufw disable command
pub fn disable() -> Result<bool> {
    run_command("ufw", None, &["disable"])
}

/// return true if ufw is active
// Be careful. systemctl status and ufw status may return different value.
#[must_use]
pub fn is_active() -> bool {
    if let Some(output) = run_command_output("ufw", None, &["status"]) {
        output.contains("Status: active")
    } else {
        false
    }
}

/// `ufw reset` command
/// # Errors
/// * fail to run ufw reset command
pub fn reset() -> Result<bool> {
    run_command("ufw", None, &["reset"])
}
