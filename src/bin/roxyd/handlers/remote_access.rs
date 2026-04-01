// TODO: Scaffolding only — implement actual remote-access logic later.

use review_protocol::types::node::{NodeRemoteAccessRequest, NodeRemoteAccessResponse};

/// Handles a node remote-access configuration request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(req: NodeRemoteAccessRequest) -> Result<NodeRemoteAccessResponse, String> {
    match req {
        NodeRemoteAccessRequest::Get => {
            unimplemented!("NodeRemoteAccessRequest::Get")
        }
        NodeRemoteAccessRequest::Set { .. } => {
            unimplemented!("NodeRemoteAccessRequest::Set")
        }
        NodeRemoteAccessRequest::Restart => {
            unimplemented!("NodeRemoteAccessRequest::Restart")
        }
    }
}
