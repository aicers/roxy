// TODO: Scaffolding only — implement actual time-sync logic later.

use review_protocol::types::node::{NodeTimeSyncRequest, NodeTimeSyncResponse};

/// Handles a node time-synchronization request.
///
/// # Errors
///
/// Returns an error message if the operation fails.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
#[allow(clippy::unused_async)]
pub async fn handle(req: NodeTimeSyncRequest) -> Result<NodeTimeSyncResponse, String> {
    match req {
        NodeTimeSyncRequest::Get => {
            unimplemented!("NodeTimeSyncRequest::Get")
        }
        NodeTimeSyncRequest::Set { .. } => {
            unimplemented!("NodeTimeSyncRequest::Set")
        }
        NodeTimeSyncRequest::Enable => {
            unimplemented!("NodeTimeSyncRequest::Enable")
        }
        NodeTimeSyncRequest::Disable => {
            unimplemented!("NodeTimeSyncRequest::Disable")
        }
        NodeTimeSyncRequest::Status => {
            unimplemented!("NodeTimeSyncRequest::Status")
        }
    }
}
