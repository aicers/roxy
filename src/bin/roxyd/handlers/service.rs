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
pub async fn handle(req: NodeServiceRequest) -> Result<NodeServiceResponse, String> {
    match req {
        NodeServiceRequest::Start { .. } => {
            unimplemented!("NodeServiceRequest::Start")
        }
        NodeServiceRequest::Stop { .. } => {
            unimplemented!("NodeServiceRequest::Stop")
        }
        NodeServiceRequest::Status { .. } => {
            unimplemented!("NodeServiceRequest::Status")
        }
        NodeServiceRequest::Restart { .. } => {
            unimplemented!("NodeServiceRequest::Restart")
        }
    }
}
