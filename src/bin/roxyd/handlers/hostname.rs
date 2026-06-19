//! Hostname request handling.

use std::sync::Arc;

use review_protocol::types::node::{NodeHostnameRequest, NodeHostnameResponse};

const ERR_FAIL: &str = "fail";

fn read_hostname() -> String {
    gethostname::gethostname().to_string_lossy().into_owned()
}

/// Performs the system hostname write operation.
trait HostnameWriter: Send + Sync {
    /// Sets the system hostname.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the hostname could not be set.
    fn set(&self, hostname: String) -> Result<(), ()>;
}

/// Production hostname writer.
struct SystemHostnameWriter;

impl HostnameWriter for SystemHostnameWriter {
    fn set(&self, hostname: String) -> Result<(), ()> {
        hostname::set(hostname).map_err(|_| ())
    }
}

/// Handles a node hostname management request.
///
/// `Get` always returns `NodeHostnameResponse::Get { hostname }` rather than a
/// read-error path (an empty string only when the blocking read task fails to join).
///
/// # Errors
///
/// Returns `"fail"` if setting the hostname fails.
pub(crate) async fn handle(req: NodeHostnameRequest) -> Result<NodeHostnameResponse, String> {
    handle_with_writer(req, Arc::new(SystemHostnameWriter)).await
}

async fn handle_with_writer(
    req: NodeHostnameRequest,
    writer: Arc<dyn HostnameWriter>,
) -> Result<NodeHostnameResponse, String> {
    match req {
        NodeHostnameRequest::Get => {
            let hostname = tokio::task::spawn_blocking(read_hostname)
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
mod mock {
    use std::sync::Mutex;

    use super::HostnameWriter;

    #[derive(Default)]
    pub(super) struct MockHostnameWriter {
        hostnames: Mutex<Vec<String>>,
        fail: bool,
    }

    impl MockHostnameWriter {
        pub(super) fn failing() -> Self {
            Self {
                hostnames: Mutex::default(),
                fail: true,
            }
        }

        pub(super) fn hostnames(&self) -> Vec<String> {
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

    #[tokio::test]
    async fn set_returns_done_on_success() {
        let writer = Arc::new(MockHostnameWriter::default());

        let response = handle_with_writer(
            NodeHostnameRequest::Set {
                hostname: "roxy-node".to_string(),
            },
            writer.clone(),
        )
        .await
        .expect("set should succeed");

        assert_eq!(response, NodeHostnameResponse::Done);
        assert_eq!(writer.hostnames(), ["roxy-node"]);
    }

    #[tokio::test]
    async fn set_returns_fail_on_write_error() {
        let writer = Arc::new(MockHostnameWriter::failing());

        let error = handle_with_writer(
            NodeHostnameRequest::Set {
                hostname: "roxy-node".to_string(),
            },
            writer,
        )
        .await
        .expect_err("set should fail");

        assert_eq!(error, ERR_FAIL);
    }
}
