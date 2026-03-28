// TODO: Scaffolding only — implement actual service-control logic later.

use review_protocol::types::node::{NodeServiceRequest, NodeServiceResponse};

/// Handles a node service-control request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(_req: NodeServiceRequest) -> Result<NodeServiceResponse, String> {
    // TODO: implement service-control mapping
    unimplemented!("node_service handler not yet implemented")
}
