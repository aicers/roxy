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
pub async fn handle(req: NodePowerRequest) -> Result<NodePowerResponse, String> {
    match req {
        NodePowerRequest::Reboot => {
            unimplemented!("NodePowerRequest::Reboot")
        }
        NodePowerRequest::Shutdown => {
            unimplemented!("NodePowerRequest::Shutdown")
        }
        NodePowerRequest::GracefulReboot => {
            unimplemented!("NodePowerRequest::GracefulReboot")
        }
        NodePowerRequest::GracefulShutdown => {
            unimplemented!("NodePowerRequest::GracefulShutdown")
        }
    }
}
