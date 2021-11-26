use serde::Deserialize;

#[derive(Deserialize)]
pub enum Task {
    Reboot,
}

impl Task {
    pub fn execute(&self) {
        match self {
            Task::Reboot => self.reboot(),
        }
    }

    fn reboot(&self) {
        nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_AUTOBOOT).expect("infallible");
    }
}
