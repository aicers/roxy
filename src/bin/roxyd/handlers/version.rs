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
pub async fn handle(_req: NodeVersionRequest) -> Result<NodeVersionResponse, String> {
    // TODO: implement version-management mapping
    unimplemented!("node_version handler not yet implemented")
}
