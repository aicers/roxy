mod hwinfo;
mod ifconfig;
mod ntp;
mod services;
mod sshd;
mod syslog;
pub(crate) mod task;
mod ufw;

use super::common::{Nic, NicOutput, SubCommand};
