use anyhow::{anyhow, Result};
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
