/// Handles the `ResourceUsage` request from the Manager.
///
/// # Panics
///
/// Always panics — scaffolding only, not yet implemented.
// TODO: Implement resource usage collection once the ResourceUsage RequestCode is available in review-protocol.
pub async fn handle() -> Result<(String, review_protocol::types::ResourceUsage), String> {
    unimplemented!("ResourceUsage handler not yet implemented")
}
