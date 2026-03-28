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
pub async fn handle(_req: NodeTimeSyncRequest) -> Result<NodeTimeSyncResponse, String> {
    // TODO: implement time-sync mapping
    unimplemented!("node_time_sync handler not yet implemented")
}
