mod interface;
mod services;

use anyhow::{Result, anyhow};
pub use interface::{Nic, NicOutput};
use serde::{Deserialize, Serialize};
pub use services::waitfor_up;

/// Types of command to node.
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub enum Node {
    Hostname(SubCommand),
    Interface(SubCommand),
    Ntp(SubCommand),
    PowerOff,
    Reboot,
    GracefulReboot,
    GracefulPowerOff,
    Service(SubCommand),
    Sshd(SubCommand),
    Syslog(SubCommand),
    Ufw(SubCommand),
    Version(SubCommand),
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NodeRequest {
    /// command
    pub kind: Node,
    /// command arguments
    pub arg: Vec<u8>,
}

impl NodeRequest {
    /// # Arguments
    ///
    /// * cmd<T>: command arguments. T: type of arguments
    ///
    /// # Errors
    ///
    /// * If serialization of arguments fails, then an error is returned.
    pub fn new<T>(kind: Node, cmd: T) -> Result<Self>
    where
        T: Serialize,
    {
        match bincode::serialize(&cmd) {
            Ok(arg) => Ok(NodeRequest { kind, arg }),
            Err(e) => Err(anyhow!("Error: {e}")),
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum SubCommand {
    Add,
    Delete,
    Disable,
    Enable,
    Get,
    Init,
    List,
    Set,
    SetOsVersion,
    SetProductVersion,
    Status,
    Update,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_request_new_serializes_arg() {
        let req = NodeRequest::new(Node::Hostname(SubCommand::Set), "roxy-host".to_string())
            .expect("NodeRequest::new should succeed");

        assert_eq!(req.kind, Node::Hostname(SubCommand::Set));

        let decoded: String =
            bincode::deserialize(&req.arg).expect("arg should decode into String");
        assert_eq!(decoded, "roxy-host");
    }

    #[test]
    fn test_node_request_new_option_arg_roundtrip() {
        let none_req = NodeRequest::new::<Option<String>>(Node::Ntp(SubCommand::Get), None)
            .expect("NodeRequest::new should succeed");
        let decoded_none: Option<String> =
            bincode::deserialize(&none_req.arg).expect("arg should decode into Option<String>");
        assert_eq!(decoded_none, None);

        let some_req = NodeRequest::new(
            Node::Ntp(SubCommand::Set),
            Some("time.example.com".to_string()),
        )
        .expect("NodeRequest::new should succeed");
        let decoded_some: Option<String> =
            bincode::deserialize(&some_req.arg).expect("arg should decode into Option<String>");
        assert_eq!(decoded_some, Some("time.example.com".to_string()));
    }

    #[test]
    fn test_node_request_new_error_path() {
        struct BadSerialize;

        impl serde::Serialize for BadSerialize {
            fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                Err(serde::ser::Error::custom("boom"))
            }
        }

        let err = NodeRequest::new(Node::Ufw(SubCommand::Get), BadSerialize)
            .expect_err("NodeRequest::new should fail");
        assert!(err.to_string().contains("boom"));
    }

    #[test]
    fn test_bincode_roundtrip_node_and_subcommand() {
        let nodes = [
            Node::Hostname(SubCommand::Get),
            Node::Interface(SubCommand::List),
            Node::Ntp(SubCommand::Status),
            Node::PowerOff,
            Node::Reboot,
            Node::GracefulReboot,
            Node::GracefulPowerOff,
            Node::Service(SubCommand::Enable),
            Node::Sshd(SubCommand::Set),
            Node::Syslog(SubCommand::Init),
            Node::Ufw(SubCommand::Add),
            Node::Version(SubCommand::SetProductVersion),
        ];

        for node in nodes {
            let encoded = bincode::serialize(&node).expect("serialize Node");
            let decoded: Node = bincode::deserialize(&encoded).expect("deserialize Node");
            assert_eq!(decoded, node);
        }

        let cmds = [
            SubCommand::Add,
            SubCommand::Delete,
            SubCommand::Disable,
            SubCommand::Enable,
            SubCommand::Get,
            SubCommand::Init,
            SubCommand::List,
            SubCommand::Set,
            SubCommand::SetOsVersion,
            SubCommand::SetProductVersion,
            SubCommand::Status,
            SubCommand::Update,
        ];

        for cmd in cmds {
            let encoded_cmd = bincode::serialize(&cmd).expect("serialize SubCommand");
            let decoded_cmd: SubCommand =
                bincode::deserialize(&encoded_cmd).expect("deserialize SubCommand");
            assert_eq!(decoded_cmd, cmd);
        }
    }
}
