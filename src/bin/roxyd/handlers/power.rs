// TODO: Scaffolding only — implement actual power-control logic later.

use review_protocol::types::node::{NodePowerRequest, NodePowerResponse};

/// Handles a node power-control request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(_req: NodePowerRequest) -> Result<NodePowerResponse, String> {
    // TODO: implement power-control mapping
    unimplemented!("node_power handler not yet implemented")
}
