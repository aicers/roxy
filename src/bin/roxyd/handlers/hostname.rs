// TODO: Scaffolding only — implement actual hostname logic later.

use review_protocol::types::node::{NodeHostnameRequest, NodeHostnameResponse};

/// Handles a node hostname management request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(req: NodeHostnameRequest) -> Result<NodeHostnameResponse, String> {
    match req {
        NodeHostnameRequest::Get => {
            unimplemented!("NodeHostnameRequest::Get")
        }
        NodeHostnameRequest::Set { .. } => {
            unimplemented!("NodeHostnameRequest::Set")
        }
    }
}
