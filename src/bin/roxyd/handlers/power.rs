//! Power-control request handling.
//!
//! Immediate [`NodePowerRequest::Reboot`] and [`NodePowerRequest::Shutdown`]
//! are fire-and-forget under review-protocol 0.19.0: the dispatch layer does
//! not send a wire response. The handler runs the destructive system call on a
//! blocking thread so legacy flat `reboot`/`shutdown` compatibility paths can
//! still return before the operation runs.
//!
//! Graceful variants spawn the platform reboot/poweroff command on a blocking
//! thread and return [`NodePowerResponse::Initiated`] on successful spawn,
//! `"fail"` otherwise.

use std::process::Command;
use std::sync::Arc;

use review_protocol::types::node::{NodePowerRequest, NodePowerResponse};

#[cfg(not(target_os = "linux"))]
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

/// Handles a [`NodePowerRequest`].
///
/// Immediate variants run the system call on a blocking thread and return
/// without waiting for it to complete. The return value is not sent on the wire
/// for grouped `NodePower` requests (review-protocol 0.19.0), but is still used
/// by legacy flat `reboot`/`shutdown` compatibility paths.
///
/// # Errors
///
/// Returns `Err("invalid command")` for immediate requests on platforms where
/// they are not supported, and `Err("fail")` if a graceful operation could not
/// be initiated.
pub(crate) async fn handle(
    req: NodePowerRequest,
    backend: Arc<dyn PowerBackend>,
) -> Result<NodePowerResponse, String> {
    match req {
        NodePowerRequest::Reboot => immediate_reboot(backend),
        NodePowerRequest::Shutdown => immediate_shutdown(backend),
        NodePowerRequest::GracefulReboot => graceful_reboot(backend).await,
        NodePowerRequest::GracefulShutdown => graceful_power_off(backend).await,
    }
}

fn immediate_reboot(backend: Arc<dyn PowerBackend>) -> Result<NodePowerResponse, String> {
    #[cfg(target_os = "linux")]
    {
        tokio::task::spawn_blocking(move || backend.reboot());
        Ok(NodePowerResponse::Initiated)
    }
    #[cfg(not(target_os = "linux"))]
    {
        drop(backend);
        Err(ERR_INVALID_COMMAND.to_string())
    }
}

fn immediate_shutdown(backend: Arc<dyn PowerBackend>) -> Result<NodePowerResponse, String> {
    #[cfg(target_os = "linux")]
    {
        tokio::task::spawn_blocking(move || backend.power_off());
        Ok(NodePowerResponse::Initiated)
    }
    #[cfg(not(target_os = "linux"))]
    {
        drop(backend);
        Err(ERR_INVALID_COMMAND.to_string())
    }
}

async fn graceful_reboot(backend: Arc<dyn PowerBackend>) -> Result<NodePowerResponse, String> {
    match tokio::task::spawn_blocking(move || backend.graceful_reboot()).await {
        Ok(Ok(())) => Ok(NodePowerResponse::Initiated),
        Ok(Err(())) | Err(_) => Err(ERR_FAIL.to_string()),
    }
}

async fn graceful_power_off(backend: Arc<dyn PowerBackend>) -> Result<NodePowerResponse, String> {
    match tokio::task::spawn_blocking(move || backend.graceful_power_off()).await {
        Ok(Ok(())) => Ok(NodePowerResponse::Initiated),
        Ok(Err(())) | Err(_) => Err(ERR_FAIL.to_string()),
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

        let resp = handle(NodePowerRequest::Reboot, mock.clone())
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

        let resp = handle(NodePowerRequest::Shutdown, mock.clone())
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

        let err = handle(NodePowerRequest::Reboot, mock.clone())
            .await
            .expect_err("reboot should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);
    }

    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn handle_shutdown_on_non_linux_returns_invalid_command() {
        let mock = Arc::new(MockPowerBackend::default());

        let err = handle(NodePowerRequest::Shutdown, mock.clone())
            .await
            .expect_err("shutdown should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(mock.power_off_count.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerBackend::default());

        let resp = handle(NodePowerRequest::GracefulReboot, mock.clone())
            .await
            .expect("graceful reboot should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_reboot_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerBackend::default());
        mock.graceful_reboot_fail.store(true, Ordering::SeqCst);

        let err = handle(NodePowerRequest::GracefulReboot, mock.clone())
            .await
            .expect_err("graceful reboot should fail");
        assert_eq!(err, ERR_FAIL);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerBackend::default());

        let resp = handle(NodePowerRequest::GracefulShutdown, mock.clone())
            .await
            .expect("graceful shutdown should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_power_off_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerBackend::default());
        mock.graceful_power_off_fail.store(true, Ordering::SeqCst);

        let err = handle(NodePowerRequest::GracefulShutdown, mock.clone())
            .await
            .expect_err("graceful shutdown should fail");
        assert_eq!(err, ERR_FAIL);
    }
}
