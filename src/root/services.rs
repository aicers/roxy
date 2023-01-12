use super::run_command_output;
use anyhow::{anyhow, Result};
use roxy::common::SubCommand;

pub fn service_control(unit: &str, cmd: SubCommand) -> Result<bool> {
    match cmd {
        SubCommand::Disable => systemctl::stop(unit)
            .map(|status| status.success())
            .map_err(Into::into),
        SubCommand::Enable | SubCommand::Update => systemctl::restart(unit)
            .map(|status| status.success())
            .map_err(Into::into),
        SubCommand::Status => systemctl::is_active(unit).map_err(Into::into),
        _ => Err(anyhow!("invalid command")),
    }
}

/// # Errors
/// * fail to run docker stop <container> command
#[allow(unused)]
fn docker_stop(container: &str) -> Result<()> {
    if run_command_output("docker", None, &["stop", container]).is_none() {
        return Err(anyhow!("failed to stop service"));
    }
    Ok(())
}

/// # Errors
/// * fail to run docker start <container> command
#[allow(unused)]
fn docker_start(container: &str) -> Result<()> {
    if run_command_output("docker", None, &["start", container]).is_none() {
        return Err(anyhow!("failed to start service"));
    }
    Ok(())
}
