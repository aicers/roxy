//! Power-control request handling.
//!
//! Immediate [`NodePowerRequest::Reboot`] and [`NodePowerRequest::Shutdown`]
//! are fire-and-forget under review-protocol 0.19.0: the dispatch layer does
//! not send a wire response. The handler spawns the destructive system call in
//! the background so legacy flat `reboot`/`shutdown` compatibility paths can
//! still return before the operation runs.
//!
//! Graceful variants spawn the platform reboot/poweroff command and return
//! [`NodePowerResponse::Initiated`] on successful spawn, `"fail"` otherwise.

use std::process::Command;
use std::sync::Arc;

use review_protocol::types::node::{NodePowerRequest, NodePowerResponse};

const ERR_INVALID_COMMAND: &str = "invalid command";
const ERR_FAIL: &str = "fail";

/// Performs the platform-specific power-control operations.
///
/// Production code uses [`SystemPowerBackend`]; tests inject a mock so that
/// power operations can be observed without actually rebooting the host.
pub(crate) trait PowerBackend: Send + Sync {
    /// Performs an immediate reboot. On Linux this calls
    /// `nix::sys::reboot::reboot`, which does not return on success.
    ///
    /// Only called from the immediate-reboot path, which is Linux-only.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn reboot(&self);

    /// Performs an immediate power-off.
    ///
    /// Only called from the immediate-shutdown path, which is Linux-only.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    fn power_off(&self);

    /// Spawns a graceful reboot process.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the process could not be spawned.
    fn graceful_reboot(&self) -> Result<(), ()>;

    /// Spawns a graceful power-off process.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the process could not be spawned.
    fn graceful_power_off(&self) -> Result<(), ()>;
}

/// Production backend that triggers reboot/power-off via `nix::sys::reboot`
/// (immediate) and the platform's standard CLI tools (graceful).
pub(crate) struct SystemPowerBackend;

impl PowerBackend for SystemPowerBackend {
    fn reboot(&self) {
        #[cfg(target_os = "linux")]
        {
            if let Err(e) = nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_AUTOBOOT) {
                tracing::error!("nix reboot failed: {e}");
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            tracing::error!("immediate reboot is not supported on this platform");
        }
    }

    fn power_off(&self) {
        #[cfg(target_os = "linux")]
        {
            if let Err(e) = nix::sys::reboot::reboot(nix::sys::reboot::RebootMode::RB_POWER_OFF) {
                tracing::error!("nix poweroff failed: {e}");
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            tracing::error!("immediate poweroff is not supported on this platform");
        }
    }

    fn graceful_reboot(&self) -> Result<(), ()> {
        #[cfg(target_os = "linux")]
        let result = Command::new("reboot").spawn();
        #[cfg(target_os = "macos")]
        let result = Command::new("sudo").args(["reboot"]).spawn();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "graceful reboot is not supported on this platform",
        ));

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::debug!("Failed to execute graceful reboot: {e}");
                Err(())
            }
        }
    }

    fn graceful_power_off(&self) -> Result<(), ()> {
        #[cfg(target_os = "linux")]
        let result = Command::new("poweroff").spawn();
        #[cfg(target_os = "macos")]
        let result = Command::new("sudo").args(["shutdown", "-h", "now"]).spawn();
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        let result: std::io::Result<std::process::Child> = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "graceful poweroff is not supported on this platform",
        ));

        match result {
            Ok(_) => Ok(()),
            Err(e) => {
                tracing::debug!("Failed to execute graceful poweroff: {e}");
                Err(())
            }
        }
    }
}

/// Per-stream handler state for power requests.
pub(crate) struct PowerHandler {
    backend: Arc<dyn PowerBackend>,
}

impl PowerHandler {
    pub(crate) fn new(backend: Arc<dyn PowerBackend>) -> Self {
        Self { backend }
    }

    /// Handles a [`NodePowerRequest`].
    ///
    /// Immediate variants spawn the system call in the background and return
    /// without waiting for it to complete. The return value is not sent on the
    /// wire for grouped `NodePower` requests (review-protocol 0.19.0), but is
    /// still used by legacy flat `reboot`/`shutdown` compatibility paths.
    ///
    /// # Errors
    ///
    /// Returns `Err("invalid command")` for immediate requests on platforms
    /// where they are not supported, and `Err("fail")` if a graceful
    /// operation could not be initiated.
    #[allow(clippy::unused_async)]
    pub(crate) async fn handle(
        &mut self,
        req: NodePowerRequest,
    ) -> Result<NodePowerResponse, String> {
        match req {
            NodePowerRequest::Reboot => self.immediate_reboot(),
            NodePowerRequest::Shutdown => self.immediate_shutdown(),
            NodePowerRequest::GracefulReboot => self.graceful_reboot(),
            NodePowerRequest::GracefulShutdown => self.graceful_power_off(),
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(clippy::unused_self))]
    fn immediate_reboot(&self) -> Result<NodePowerResponse, String> {
        #[cfg(target_os = "linux")]
        {
            let backend = self.backend.clone();
            tokio::spawn(async move {
                backend.reboot();
            });
            Ok(NodePowerResponse::Initiated)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ERR_INVALID_COMMAND.to_string())
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(clippy::unused_self))]
    fn immediate_shutdown(&self) -> Result<NodePowerResponse, String> {
        #[cfg(target_os = "linux")]
        {
            let backend = self.backend.clone();
            tokio::spawn(async move {
                backend.power_off();
            });
            Ok(NodePowerResponse::Initiated)
        }
        #[cfg(not(target_os = "linux"))]
        {
            Err(ERR_INVALID_COMMAND.to_string())
        }
    }

    fn graceful_reboot(&self) -> Result<NodePowerResponse, String> {
        match self.backend.graceful_reboot() {
            Ok(()) => Ok(NodePowerResponse::Initiated),
            Err(()) => Err(ERR_FAIL.to_string()),
        }
    }

    fn graceful_power_off(&self) -> Result<NodePowerResponse, String> {
        match self.backend.graceful_power_off() {
            Ok(()) => Ok(NodePowerResponse::Initiated),
            Err(()) => Err(ERR_FAIL.to_string()),
        }
    }
}

#[cfg(test)]
pub(crate) use mock::MockPowerBackend;

#[cfg(test)]
pub(crate) mod mock {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::PowerBackend;

    /// In-memory mock backend used by tests. Records call counts and can
    /// be configured to fail graceful operations.
    #[derive(Default)]
    pub(crate) struct MockPowerBackend {
        pub reboot_count: AtomicUsize,
        pub power_off_count: AtomicUsize,
        pub graceful_reboot_count: AtomicUsize,
        pub graceful_power_off_count: AtomicUsize,
        pub graceful_reboot_fail: AtomicBool,
        pub graceful_power_off_fail: AtomicBool,
    }

    impl PowerBackend for MockPowerBackend {
        fn reboot(&self) {
            self.reboot_count.fetch_add(1, Ordering::SeqCst);
        }

        fn power_off(&self) {
            self.power_off_count.fetch_add(1, Ordering::SeqCst);
        }

        fn graceful_reboot(&self) -> Result<(), ()> {
            self.graceful_reboot_count.fetch_add(1, Ordering::SeqCst);
            if self.graceful_reboot_fail.load(Ordering::SeqCst) {
                Err(())
            } else {
                Ok(())
            }
        }

        fn graceful_power_off(&self) -> Result<(), ()> {
            self.graceful_power_off_count.fetch_add(1, Ordering::SeqCst);
            if self.graceful_power_off_fail.load(Ordering::SeqCst) {
                Err(())
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::mock::MockPowerBackend;
    use super::*;

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn handle_reboot_on_linux_spawns_immediate_action() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let resp = handler
            .handle(NodePowerRequest::Reboot)
            .await
            .expect("reboot should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);

        for _ in 0..50 {
            if mock.reboot_count.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn handle_shutdown_on_linux_spawns_immediate_action() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let resp = handler
            .handle(NodePowerRequest::Shutdown)
            .await
            .expect("shutdown should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);

        for _ in 0..50 {
            if mock.power_off_count.load(Ordering::SeqCst) > 0 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(mock.power_off_count.load(Ordering::SeqCst), 1);
    }

    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn handle_reboot_on_non_linux_returns_invalid_command() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let err = handler
            .handle(NodePowerRequest::Reboot)
            .await
            .expect_err("reboot should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);
    }

    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn handle_shutdown_on_non_linux_returns_invalid_command() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let err = handler
            .handle(NodePowerRequest::Shutdown)
            .await
            .expect_err("shutdown should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(mock.power_off_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let resp = handler
            .handle(NodePowerRequest::GracefulReboot)
            .await
            .expect("graceful reboot should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_reboot_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerBackend::default());
        mock.graceful_reboot_fail.store(true, Ordering::SeqCst);
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let err = handler
            .handle(NodePowerRequest::GracefulReboot)
            .await
            .expect_err("graceful reboot should fail");
        assert_eq!(err, ERR_FAIL);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerBackend::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let resp = handler
            .handle(NodePowerRequest::GracefulShutdown)
            .await
            .expect("graceful shutdown should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_power_off_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerBackend::default());
        mock.graceful_power_off_fail.store(true, Ordering::SeqCst);
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerBackend>);

        let err = handler
            .handle(NodePowerRequest::GracefulShutdown)
            .await
            .expect_err("graceful shutdown should fail");
        assert_eq!(err, ERR_FAIL);
    }
}
