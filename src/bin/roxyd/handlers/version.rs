// TODO: Scaffolding only — implement actual version-management logic later.

use review_protocol::types::node::{NodeVersionRequest, NodeVersionResponse};

/// Handles a node version-management request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(req: NodeVersionRequest) -> Result<NodeVersionResponse, String> {
    match req {
        NodeVersionRequest::Get => {
            unimplemented!("NodeVersionRequest::Get")
        }
        NodeVersionRequest::SetOsVersion { .. } => {
            unimplemented!("NodeVersionRequest::SetOsVersion")
        }
        NodeVersionRequest::SetProductVersion { .. } => {
            unimplemented!("NodeVersionRequest::SetProductVersion")
        }
    }
}
