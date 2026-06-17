//! Hostname request handling.

use std::ffi::OsString;
use std::sync::Arc;

use review_protocol::types::node::{NodeHostnameRequest, NodeHostnameResponse};

const ERR_FAIL: &str = "fail";

fn read_hostname_or_default() -> String {
    hostname_or_default(hostname::get().ok())
}

fn hostname_or_default(hostname: Option<OsString>) -> String {
    match hostname {
        Some(hostname) => hostname.to_string_lossy().into_owned(),
        None => String::new(),
    }
}

/// Performs the system hostname write operation.
pub(crate) trait HostnameWriter: Send + Sync {
    /// Sets the system hostname.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the hostname could not be set.
    fn set(&self, hostname: String) -> Result<(), ()>;
}

/// Production hostname writer.
pub(crate) struct SystemHostnameWriter;

impl HostnameWriter for SystemHostnameWriter {
    fn set(&self, hostname: String) -> Result<(), ()> {
        hostname::set(hostname).map_err(|_| ())
    }
}

fn writer_for_call() -> Arc<dyn HostnameWriter> {
    #[cfg(test)]
    if let Some(writer) = test_support::current_writer() {
        return writer;
    }
    Arc::new(SystemHostnameWriter)
}

/// Handles a node hostname management request.
///
/// `Get` always returns `NodeHostnameResponse::Get { hostname }` rather than a
/// read-error path (an empty string when the hostname cannot be read).
///
/// # Errors
///
/// Returns `"fail"` if setting the hostname fails.
pub(crate) async fn handle(req: NodeHostnameRequest) -> Result<NodeHostnameResponse, String> {
    let writer = writer_for_call();
    match req {
        NodeHostnameRequest::Get => {
            let hostname = tokio::task::spawn_blocking(read_hostname_or_default)
                .await
                .unwrap_or_default();
            Ok(NodeHostnameResponse::Get { hostname })
        }
        NodeHostnameRequest::Set { hostname } => {
            match tokio::task::spawn_blocking(move || writer.set(hostname)).await {
                Ok(Ok(())) => Ok(NodeHostnameResponse::Done),
                Ok(Err(())) | Err(_) => Err(ERR_FAIL.to_string()),
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Arc, Mutex, OnceLock};

    use super::HostnameWriter;

    fn slot() -> &'static Mutex<Option<Arc<dyn HostnameWriter>>> {
        static SLOT: OnceLock<Mutex<Option<Arc<dyn HostnameWriter>>>> = OnceLock::new();
        SLOT.get_or_init(|| Mutex::new(None))
    }

    pub(crate) fn current_writer() -> Option<Arc<dyn HostnameWriter>> {
        slot()
            .lock()
            .expect("hostname writer override lock")
            .clone()
    }

    pub(crate) struct WriterOverrideGuard;

    impl Drop for WriterOverrideGuard {
        fn drop(&mut self) {
            *slot().lock().expect("hostname writer override lock") = None;
        }
    }

    pub(crate) fn override_writer(writer: Arc<dyn HostnameWriter>) -> WriterOverrideGuard {
        *slot().lock().expect("hostname writer override lock") = Some(writer);
        WriterOverrideGuard
    }
}

#[cfg(test)]
pub(crate) mod mock {
    use std::sync::Mutex;

    use super::HostnameWriter;

    #[derive(Default)]
    pub(crate) struct MockHostnameWriter {
        hostnames: Mutex<Vec<String>>,
        fail: bool,
    }

    impl MockHostnameWriter {
        pub(crate) fn failing() -> Self {
            Self {
                hostnames: Mutex::default(),
                fail: true,
            }
        }

        pub(crate) fn hostnames(&self) -> Vec<String> {
            self.hostnames.lock().expect("hostname lock").clone()
        }
    }

    impl HostnameWriter for MockHostnameWriter {
        fn set(&self, hostname: String) -> Result<(), ()> {
            self.hostnames.lock().expect("hostname lock").push(hostname);
            if self.fail { Err(()) } else { Ok(()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::mock::MockHostnameWriter;
    use super::*;

    #[test]
    fn hostname_or_default_returns_empty_string_on_none() {
        assert_eq!(hostname_or_default(None), "");
    }

    #[cfg(unix)]
    #[test]
    fn hostname_or_default_converts_non_utf8_with_lossy_replacement() {
        use std::os::unix::ffi::OsStringExt;

        let non_utf8 = OsString::from_vec(vec![0xff, 0xfe]);
        assert_eq!(hostname_or_default(Some(non_utf8)), "\u{fffd}\u{fffd}");
    }

    #[tokio::test]
    async fn get_returns_current_hostname() {
        let expected = read_hostname_or_default();

        let response = handle(NodeHostnameRequest::Get)
            .await
            .expect("get should succeed");

        assert_eq!(response, NodeHostnameResponse::Get { hostname: expected });
    }

    #[tokio::test]
    async fn set_returns_done_on_success() {
        let writer = Arc::new(MockHostnameWriter::default());
        let _guard = test_support::override_writer(writer.clone());

        let response = handle(NodeHostnameRequest::Set {
            hostname: "roxy-node".to_string(),
        })
        .await
        .expect("set should succeed");

        assert_eq!(response, NodeHostnameResponse::Done);
        assert_eq!(writer.hostnames(), ["roxy-node"]);
    }

    #[tokio::test]
    async fn set_returns_fail_on_write_error() {
        let writer = Arc::new(MockHostnameWriter::failing());
        let _guard = test_support::override_writer(writer);

        let error = handle(NodeHostnameRequest::Set {
            hostname: "roxy-node".to_string(),
        })
        .await
        .expect_err("set should fail");

        assert_eq!(error, ERR_FAIL);
    }
}
