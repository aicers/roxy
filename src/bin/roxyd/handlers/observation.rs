// TODO: Scaffolding only — implement actual observation logic later.

use review_protocol::types::node::{NodeObservationRequest, NodeObservationResponse};

/// Handles a node host-observation request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(_req: NodeObservationRequest) -> Result<NodeObservationResponse, String> {
    // TODO: implement observation mapping
    unimplemented!("node_observation handler not yet implemented")
}
