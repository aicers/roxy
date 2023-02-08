use super::services::service_control;
use anyhow::Result;
use regex::Regex;
use roxy::common::DEFAULT_PATH_ENV;
use std::process::Command;
pub type AccessLists = Vec<(String, String, String, Option<String>, Option<String>)>;

const UFW_UNIT: &str = "ufw";

// Gets firewall rules. The result of command `ufw status`.
// Currently only return IPv4 rules.
//
// # Return
//
// Vec<(Action, From, To, Protocol, Interface)>
// * Action: String,        // ALLOW IN, ALLOW OUT, DENY IN, DENY OUT
// * From: String,          // Source address, port (or "Any")
// * To: String,            // Destination address, port (or "Any")
// * Protocol: Option<String>  // Protocol name
// * Interface: Option<String> // Interface name
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
// # Errors
//
// * fail to execute ufw status command
// * fail to compile regex to parse ufw rules
pub(crate) fn get() -> Result<Option<AccessLists>> {
    if let Some(output) = run_ufw_output(&["status"]) {
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

// Adds new rules.
// This function execute `ufw add` command internally.
//
// # Example
//
// UFW rule syntax
// allow|deny [in on <dev>] [from <src>] [to <dst>] [port <port>] [proto <protocol>]
// let rules_to_add = vec![
//     "allow in on eth0 to any port 80 proto tcp".to_string(),
//     "allow in on eth0 from 203.0.113.102".to_string(),
//     "allow from 203.0.113.0/24 to any port 22 proto tcp".to_string()
// ];
// ufw::add(&rules_to_add).unwrap();
//
// # Errors
//
// * fail to run ufw command
pub(crate) fn add(args: &[String]) -> Result<()> {
    for rule in args {
        run_ufw(&[rule.as_str()])?;
    }
    Ok(())
}

// Removes rules.
// This function execute `ufw delete` command internally.
//
// # Example
//
// let rules_to_delete = vec![
//     "allow from 203.0.113.101".to_string(),
//     "allow from 203.0.113.0/24 proto tcp to any port 22".to_string()];
// ufw::delete(&rules_to_delete).unwrap();
//
// # Errors
//
// * fail to run ufw delete command
pub(crate) fn delete(args: &[String]) -> Result<()> {
    for rule_id in args {
        run_ufw(&["delete", rule_id])?;
    }

    Ok(())
}

// Enables ufw
// The default ufw service must be re-registered to the system by the updated guide.
//
// # Errors
//
// * fail to run ufw enable command
pub(crate) fn enable() -> Result<bool> {
    service_control(UFW_UNIT, roxy::common::SubCommand::Enable)
}

// Disables ufw
// The default ufw service must be re-registered to the system by the updated guide.
//
// # Errors
//
// * fail to run ufw disable command
pub(crate) fn disable() -> Result<bool> {
    service_control(UFW_UNIT, roxy::common::SubCommand::Disable)
}

// Returns true if ufw is active
// Be careful. systemctl status and ufw status may return different value.
// To clear this issue, ufw must be re-registered to the system by the updated guide.
#[must_use]
pub(crate) fn is_active() -> bool {
    service_control(UFW_UNIT, roxy::common::SubCommand::Status).map_or(false, |ret| ret)
}

// `ufw reset` command
//
// # Errors
//
// * fail to run ufw reset command
pub(crate) fn reset() -> Result<bool> {
    run_ufw(&["reset"])
}

fn run_ufw(args: &[&str]) -> Result<bool> {
    let mut cmd = Command::new(UFW_UNIT);
    cmd.env("PATH", DEFAULT_PATH_ENV);
    for arg in args {
        if !arg.is_empty() {
            cmd.arg(arg);
        }
    }
    match cmd.status() {
        Ok(status) => Ok(status.success()),
        Err(e) => Err(e.into()),
    }
}

fn run_ufw_output(args: &[&str]) -> Option<String> {
    let mut cmd = Command::new(UFW_UNIT);
    cmd.env("PATH", DEFAULT_PATH_ENV);
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
