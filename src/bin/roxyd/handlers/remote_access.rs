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
pub async fn handle(_req: NodeRemoteAccessRequest) -> Result<NodeRemoteAccessResponse, String> {
    // TODO: implement remote-access mapping
    unimplemented!("node_remote_access handler not yet implemented")
}
