use anyhow::{Result, anyhow};
use roxy::common::SubCommand;

pub fn service_control(unit: &str, cmd: SubCommand) -> Result<bool> {
    let systemctl = systemctl::SystemCtl::default();

    match cmd {
        SubCommand::Disable => systemctl
            .stop(unit)
            .map(|status| status.success())
            .map_err(Into::into),
        SubCommand::Enable | SubCommand::Update => systemctl
            .restart(unit)
            .map(|status| status.success())
            .map_err(Into::into),
        SubCommand::Status => systemctl.is_active(unit).map_err(Into::into),
        _ => Err(anyhow!("invalid command")),
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    fn missing_unit_name() -> String {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos();
        format!("roxy-test-missing-{}-{nanos}.service", std::process::id())
    }

    fn assert_supported_command_result(result: Result<bool>) {
        if let Err(err) = result {
            assert_ne!(err.to_string(), "invalid command");
            assert!(err.downcast_ref::<io::Error>().is_some());
        }
    }

    #[test]
    fn test_service_control_rejects_all_unsupported_subcommands() {
        let cmds = [
            SubCommand::Add,
            SubCommand::Delete,
            SubCommand::Get,
            SubCommand::Init,
            SubCommand::List,
            SubCommand::Set,
            SubCommand::SetOsVersion,
            SubCommand::SetProductVersion,
        ];

        for cmd in cmds {
            let err = service_control("roxy-test.service", cmd)
                .expect_err("unsupported subcommand should fail");
            assert_eq!(err.to_string(), "invalid command");
        }
    }

    #[test]
    fn test_service_control_handles_all_supported_subcommands() {
        let unit = missing_unit_name();
        let cmds = [
            SubCommand::Disable,
            SubCommand::Enable,
            SubCommand::Update,
            SubCommand::Status,
        ];

        for cmd in cmds {
            assert_supported_command_result(service_control(&unit, cmd));
        }
    }
}
