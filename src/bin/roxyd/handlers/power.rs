//! Power-control request handling.
//!
//! Immediate `Reboot` and `Shutdown` requests are split into two phases: the
//! request handler *prepares* a [`PendingPowerOperation`] and returns
//! `NodePowerResponse::Initiated` without actually rebooting or powering off.
//! The dispatch layer is responsible for [releasing](PowerHandler::release_pending)
//! the pending operation only after the success response has been written
//! successfully. If the response write fails, the pending operation is dropped
//! and the destructive system call is never made.
//!
//! Graceful variants spawn the standard platform reboot/poweroff command and
//! return `Initiated` on successful spawn, `"fail"` otherwise.

use std::process::Command;
use std::sync::Arc;

use review_protocol::types::node::{NodePowerRequest, NodePowerResponse};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

const ERR_INVALID_COMMAND: &str = "invalid command";
const ERR_FAIL: &str = "fail";

/// Executes the platform-specific power-control operations.
///
/// Production code uses [`SystemPowerExecutor`]; tests inject a mock so that
/// power operations can be observed without actually rebooting the host.
pub(crate) trait PowerExecutor: Send + Sync {
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

/// Production executor that triggers reboot/power-off via `nix::sys::reboot`
/// (immediate) and the platform's standard CLI tools (graceful).
pub(crate) struct SystemPowerExecutor;

impl PowerExecutor for SystemPowerExecutor {
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

/// Which immediate power action a [`PendingPowerOperation`] represents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PowerAction {
    Reboot,
    PowerOff,
}

/// An immediate reboot or power-off intent that has been prepared but not
/// yet executed.
///
/// The pending operation owns a oneshot release channel and a background
/// task that waits on the channel before calling the destructive system
/// operation. If the operation is dropped without being released (for
/// example, because the response write failed), the sender side of the
/// channel is dropped, the background task observes the closed channel,
/// and the executor is never invoked.
pub(crate) struct PendingPowerOperation {
    release_tx: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

impl PendingPowerOperation {
    /// Prepares a pending operation. Spawns a background task that waits
    /// for the release signal before calling the executor.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub(crate) fn prepare(executor: Arc<dyn PowerExecutor>, action: PowerAction) -> Self {
        let (release_tx, release_rx) = oneshot::channel::<()>();
        let task = tokio::spawn(async move {
            if release_rx.await.is_ok() {
                match action {
                    PowerAction::Reboot => executor.reboot(),
                    PowerAction::PowerOff => executor.power_off(),
                }
            }
        });
        Self { release_tx, task }
    }

    /// Releases the pending operation. The background task receives the
    /// signal and proceeds to call the executor. The returned
    /// [`JoinHandle`] resolves once the executor returns; tests can await
    /// it to observe the call. Production callers may ignore the handle.
    pub(crate) fn release(self) -> JoinHandle<()> {
        let _ = self.release_tx.send(());
        self.task
    }
}

/// Per-stream handler state for power requests.
///
/// Owns a shared [`PowerExecutor`] and accumulates pending immediate
/// operations during a request. The dispatch layer must call
/// [`release_pending`](Self::release_pending) only after the success
/// response has been written successfully on the request stream.
pub(crate) struct PowerHandler {
    executor: Arc<dyn PowerExecutor>,
    pending: Vec<PendingPowerOperation>,
}

impl PowerHandler {
    pub(crate) fn new(executor: Arc<dyn PowerExecutor>) -> Self {
        Self {
            executor,
            pending: Vec::new(),
        }
    }

    /// Handles a [`NodePowerRequest`].
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
            NodePowerRequest::Reboot => self.prepare_immediate(PowerAction::Reboot),
            NodePowerRequest::Shutdown => self.prepare_immediate(PowerAction::PowerOff),
            NodePowerRequest::GracefulReboot => self.graceful_reboot(),
            NodePowerRequest::GracefulShutdown => self.graceful_power_off(),
        }
    }

    #[allow(clippy::unused_self)]
    fn prepare_immediate(&mut self, action: PowerAction) -> Result<NodePowerResponse, String> {
        #[cfg(target_os = "linux")]
        {
            let op = PendingPowerOperation::prepare(self.executor.clone(), action);
            self.pending.push(op);
            Ok(NodePowerResponse::Initiated)
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = action;
            Err(ERR_INVALID_COMMAND.to_string())
        }
    }

    fn graceful_reboot(&self) -> Result<NodePowerResponse, String> {
        match self.executor.graceful_reboot() {
            Ok(()) => Ok(NodePowerResponse::Initiated),
            Err(()) => Err(ERR_FAIL.to_string()),
        }
    }

    fn graceful_power_off(&self) -> Result<NodePowerResponse, String> {
        match self.executor.graceful_power_off() {
            Ok(()) => Ok(NodePowerResponse::Initiated),
            Err(()) => Err(ERR_FAIL.to_string()),
        }
    }

    /// Releases all pending immediate power operations.
    ///
    /// Returns the per-operation [`JoinHandle`]s so that callers (typically
    /// tests) can await the executor calls. Production callers may drop the
    /// returned vector.
    pub(crate) fn release_pending(&mut self) -> Vec<JoinHandle<()>> {
        self.pending
            .drain(..)
            .map(PendingPowerOperation::release)
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
pub(crate) use mock::MockPowerExecutor;

#[cfg(test)]
pub(crate) mod mock {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::PowerExecutor;

    /// In-memory mock executor used by tests. Records call counts and can
    /// be configured to fail graceful operations.
    #[derive(Default)]
    pub(crate) struct MockPowerExecutor {
        pub reboot_count: AtomicUsize,
        pub power_off_count: AtomicUsize,
        pub graceful_reboot_count: AtomicUsize,
        pub graceful_power_off_count: AtomicUsize,
        pub graceful_reboot_fail: AtomicBool,
        pub graceful_power_off_fail: AtomicBool,
    }

    impl PowerExecutor for MockPowerExecutor {
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

    use super::mock::MockPowerExecutor;
    use super::*;

    #[tokio::test]
    async fn pending_operation_executes_only_after_release() {
        let mock = Arc::new(MockPowerExecutor::default());
        let executor: Arc<dyn PowerExecutor> = mock.clone();
        let pending = PendingPowerOperation::prepare(executor, PowerAction::Reboot);

        // Give the background task a chance to run; it should still be
        // waiting on the release channel.
        tokio::task::yield_now().await;
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);

        let task = pending.release();
        task.await.expect("background task should complete");
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn pending_operation_dropped_without_release_does_not_execute() {
        let mock = Arc::new(MockPowerExecutor::default());
        let executor: Arc<dyn PowerExecutor> = mock.clone();
        let pending = PendingPowerOperation::prepare(executor, PowerAction::PowerOff);

        // Drop without releasing. The background task should observe the
        // closed sender and exit without calling the executor.
        drop(pending);

        // Yield repeatedly to allow the background task to finish.
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);
        assert_eq!(mock.power_off_count.load(Ordering::SeqCst), 0);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn handle_reboot_on_linux_prepares_pending_op() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let resp = handler
            .handle(NodePowerRequest::Reboot)
            .await
            .expect("reboot should return Initiated");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(handler.pending_count(), 1);
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);

        let handles = handler.release_pending();
        for handle in handles {
            handle.await.expect("background task should complete");
        }
        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn handle_shutdown_on_linux_prepares_pending_op() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let resp = handler
            .handle(NodePowerRequest::Shutdown)
            .await
            .expect("shutdown should return Initiated");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(handler.pending_count(), 1);

        let handles = handler.release_pending();
        for handle in handles {
            handle.await.expect("background task should complete");
        }
        assert_eq!(mock.power_off_count.load(Ordering::SeqCst), 1);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn handle_reboot_dropped_without_release_does_not_reboot() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let _ = handler
            .handle(NodePowerRequest::Reboot)
            .await
            .expect("reboot should return Initiated");

        // Simulate a response-write failure by dropping the handler without
        // calling release_pending.
        drop(handler);

        for _ in 0..10 {
            tokio::task::yield_now().await;
        }

        assert_eq!(mock.reboot_count.load(Ordering::SeqCst), 0);
    }

    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn handle_reboot_on_non_linux_returns_invalid_command() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let err = handler
            .handle(NodePowerRequest::Reboot)
            .await
            .expect_err("reboot should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(handler.pending_count(), 0);
    }

    #[cfg(not(target_os = "linux"))]
    #[tokio::test]
    async fn handle_shutdown_on_non_linux_returns_invalid_command() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let err = handler
            .handle(NodePowerRequest::Shutdown)
            .await
            .expect_err("shutdown should be unsupported on non-Linux");
        assert_eq!(err, ERR_INVALID_COMMAND);
        assert_eq!(handler.pending_count(), 0);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let resp = handler
            .handle(NodePowerRequest::GracefulReboot)
            .await
            .expect("graceful reboot should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_reboot_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_reboot_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerExecutor::default());
        mock.graceful_reboot_fail.store(true, Ordering::SeqCst);
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let err = handler
            .handle(NodePowerRequest::GracefulReboot)
            .await
            .expect_err("graceful reboot should fail");
        assert_eq!(err, ERR_FAIL);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_initiated_on_success() {
        let mock = Arc::new(MockPowerExecutor::default());
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let resp = handler
            .handle(NodePowerRequest::GracefulShutdown)
            .await
            .expect("graceful shutdown should succeed");
        assert_eq!(resp, NodePowerResponse::Initiated);
        assert_eq!(mock.graceful_power_off_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn handle_graceful_shutdown_returns_fail_on_spawn_error() {
        let mock = Arc::new(MockPowerExecutor::default());
        mock.graceful_power_off_fail.store(true, Ordering::SeqCst);
        let mut handler = PowerHandler::new(mock.clone() as Arc<dyn PowerExecutor>);

        let err = handler
            .handle(NodePowerRequest::GracefulShutdown)
            .await
            .expect_err("graceful shutdown should fail");
        assert_eq!(err, ERR_FAIL);
    }
}
