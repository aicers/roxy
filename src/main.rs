mod root;

use std::{
    io::{stdin, stdout},
    process,
};

use data_encoding::BASE64;
use root::task::{ExecResult, Task, ERR_INVALID_COMMAND};
use roxy::common::{self, Node, NodeRequest};

fn main() {
    let nr: NodeRequest = match serde_json::from_reader(stdin()) {
        Ok(nr) => nr,
        Err(err) => {
            log::error!("Command Error: {err}");
            if let Err(err) =
                serde_json::to_writer_pretty(stdout(), &ExecResult::Err(ERR_INVALID_COMMAND))
            {
                log::error!("Serialize Error: {err}");
            }
            process::exit(1);
        }
    };

    let arg = BASE64.encode(&nr.arg);
    let task = match nr.kind {
        Node::Hostname(cmd) => Task::Hostname { cmd, arg },
        Node::Interface(cmd) => Task::Interface { cmd, arg },
        Node::Ntp(cmd) => Task::Ntp { cmd, arg },
        Node::PowerOff => Task::PowerOff(arg),
        Node::Reboot => Task::Reboot(arg),
        Node::GracefulReboot => Task::GracefulReboot(arg),
        Node::GracefulPowerOff => Task::GracefulPowerOff(arg),
        Node::Service(cmd) => Task::Service { cmd, arg },
        Node::Sshd(cmd) => Task::Sshd { cmd, arg },
        Node::Syslog(cmd) => Task::Syslog { cmd, arg },
        Node::Ufw(cmd) => Task::Ufw { cmd, arg },
        Node::Version(cmd) => Task::Version { cmd, arg },
    };

    let ret = task.execute();
    if let Err(err) = serde_json::to_writer_pretty(stdout(), &ret) {
        log::error!("Stdout Error: {err}");
        process::exit(1);
    }
}
