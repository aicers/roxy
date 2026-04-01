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
    req: NodeNetworkInterfaceRequest,
) -> Result<NodeNetworkInterfaceResponse, String> {
    match req {
        NodeNetworkInterfaceRequest::List { .. } => {
            unimplemented!("NodeNetworkInterfaceRequest::List")
        }
        NodeNetworkInterfaceRequest::Get { .. } => {
            unimplemented!("NodeNetworkInterfaceRequest::Get")
        }
        NodeNetworkInterfaceRequest::ResetConfig { .. } => {
            unimplemented!("NodeNetworkInterfaceRequest::ResetConfig")
        }
        NodeNetworkInterfaceRequest::Set { .. } => {
            unimplemented!("NodeNetworkInterfaceRequest::Set")
        }
        NodeNetworkInterfaceRequest::Remove { .. } => {
            unimplemented!("NodeNetworkInterfaceRequest::Remove")
        }
    }
}
