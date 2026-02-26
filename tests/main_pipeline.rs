use std::io::Write;
use std::process::{Command, Output, Stdio};

use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use nix::unistd::pipe;
use roxy::common::{Node, NodeRequest, SubCommand};
use serde_json::json;

const ERR_INVALID_COMMAND: &str = "invalid command";

fn run_roxy(input: &[u8]) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_roxy"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn roxy");

    child
        .stdin
        .as_mut()
        .expect("missing child stdin")
        .write_all(input)
        .expect("failed to write stdin");

    child.wait_with_output().expect("failed to wait")
}

fn run_roxy_with_stdout(input: &[u8], stdout: Stdio) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_roxy"))
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn roxy");

    child
        .stdin
        .as_mut()
        .expect("missing child stdin")
        .write_all(input)
        .expect("failed to write stdin");

    child.wait_with_output().expect("failed to wait")
}

fn broken_pipe_stdout() -> Stdio {
    let (read_end, write_end) = pipe().expect("failed to create pipe");
    // Set CLOEXEC on the read end so that other tests' Command::spawn
    // calls cannot inherit it, which would keep the pipe alive and
    // prevent the expected EPIPE.
    fcntl(&read_end, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))
        .expect("failed to set CLOEXEC on read end");
    drop(read_end);
    Stdio::from(write_end)
}

fn parse_output(stdout: &[u8]) -> serde_json::Value {
    serde_json::from_slice(stdout).expect("stdout should be JSON")
}

fn parse_err(stdout: &[u8]) -> String {
    let json = parse_output(stdout);
    json.get("Err")
        .and_then(serde_json::Value::as_str)
        .expect("stdout should contain Err string")
        .to_string()
}

#[test]
fn invalid_json_returns_invalid_command_and_non_zero_exit() {
    let output = run_roxy(br#"{"kind":"PowerOff""#);
    assert!(!output.status.success());
    assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
}

#[test]
fn missing_required_field_returns_invalid_command_and_non_zero_exit() {
    let output = run_roxy(br#"{"kind":"PowerOff"}"#);
    assert!(!output.status.success());
    assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
}

#[test]
fn empty_input_returns_invalid_command_and_non_zero_exit() {
    let output = run_roxy(b"");
    assert!(!output.status.success());
    assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
}

#[test]
fn invalid_kind_variant_returns_invalid_command_and_non_zero_exit() {
    let output = run_roxy(br#"{"kind":"Unknown","arg":[]}"#);
    assert!(!output.status.success());
    assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
}

#[test]
fn invalid_arg_type_returns_invalid_command_and_non_zero_exit() {
    let output = run_roxy(br#"{"kind":"PowerOff","arg":"not_array"}"#);
    assert!(!output.status.success());
    assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
}

#[test]
fn valid_json_request_reaches_task_execution_path() {
    let request = NodeRequest::new(Node::Version(SubCommand::Get), Option::<String>::None)
        .expect("failed to build request");
    let input = serde_json::to_vec(&request).expect("failed to serialize request");
    let output = run_roxy(&input);

    assert!(output.status.success());
    let json = parse_output(&output.stdout);
    assert!(json.get("Ok").is_some() || json.get("Err").is_some());
}

#[test]
fn all_subcommand_kind_variants_reach_task_and_return_json() {
    let kinds = [
        Node::Hostname(SubCommand::Add),
        Node::Interface(SubCommand::Add),
        Node::Ntp(SubCommand::Add),
        Node::Service(SubCommand::Add),
        Node::Sshd(SubCommand::Add),
        Node::Syslog(SubCommand::Add),
        Node::Ufw(SubCommand::Get),
        Node::Version(SubCommand::Get),
    ];

    for kind in kinds {
        let request =
            NodeRequest::new(kind, Option::<String>::None).expect("failed to build request");
        let input = serde_json::to_vec(&request).expect("failed to serialize request");
        let output = run_roxy(&input);

        assert!(output.status.success());
        let json = parse_output(&output.stdout);
        assert!(json.get("Ok").is_some() || json.get("Err").is_some());
    }
}

#[test]
// Shutdown kinds are tested via parse-error path to avoid CI shutdown/reboot risk.
fn all_shutdown_kind_variants_are_covered_via_parse_error_path() {
    let shutdown_kinds = ["PowerOff", "Reboot", "GracefulReboot", "GracefulPowerOff"];

    for kind in shutdown_kinds {
        let input = serde_json::to_vec(&json!({
            "kind": kind,
            "arg": "not_array"
        }))
        .expect("failed to serialize input");
        let output = run_roxy(&input);

        assert!(!output.status.success());
        assert_eq!(parse_err(&output.stdout), ERR_INVALID_COMMAND);
    }
}

#[test]
fn parse_error_path_is_covered_when_stdout_write_fails() {
    let output = run_roxy_with_stdout(br#"{"kind":"PowerOff""#, broken_pipe_stdout());
    assert!(!output.status.success());
}

#[test]
fn execute_result_path_is_covered_when_stdout_write_fails() {
    let request = NodeRequest::new(Node::Version(SubCommand::Get), Option::<String>::None)
        .expect("failed to build request");
    let input = serde_json::to_vec(&request).expect("failed to serialize request");
    let output = run_roxy_with_stdout(&input, broken_pipe_stdout());

    assert!(!output.status.success());
}
