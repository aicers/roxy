// TODO: Scaffolding only — implement actual network-interface logic later.

use review_protocol::types::node::{NodeNetworkInterfaceRequest, NodeNetworkInterfaceResponse};

/// Handles a node network-interface management request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(
    _req: NodeNetworkInterfaceRequest,
) -> Result<NodeNetworkInterfaceResponse, String> {
    // TODO: implement network-interface mapping
    unimplemented!("node_network_interface handler not yet implemented")
}
