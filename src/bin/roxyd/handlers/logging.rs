// TODO: Scaffolding only — implement actual logging-config logic later.

use review_protocol::types::node::{NodeLoggingRequest, NodeLoggingResponse};

/// Handles a node logging-configuration request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(req: NodeLoggingRequest) -> Result<NodeLoggingResponse, String> {
    match req {
        NodeLoggingRequest::Get => {
            unimplemented!("NodeLoggingRequest::Get")
        }
        NodeLoggingRequest::Set { .. } => {
            unimplemented!("NodeLoggingRequest::Set")
        }
        NodeLoggingRequest::Clear => {
            unimplemented!("NodeLoggingRequest::Clear")
        }
        NodeLoggingRequest::Restart => {
            unimplemented!("NodeLoggingRequest::Restart")
        }
    }
}
