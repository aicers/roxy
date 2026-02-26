//! Review-protocol client connection for Manager communication.
//!
//! This module provides the client-side connection skeleton for communicating
//! with the Manager via the review-protocol. Currently a scaffolding
//! implementation—all request handlers fail with `unimplemented!()`.

use std::net::SocketAddr;

use anyhow::{Context, Result};
use review_protocol::client::ConnectionBuilder;

/// The protocol version this client supports. Must be compatible with the
/// Manager's version requirement.
const PROTOCOL_VERSION: &str = "0.16.0";

/// A connection to the Manager via review-protocol.
///
/// Owns the underlying QUIC/mTLS connection and related state.
pub struct Connection {
    inner: review_protocol::client::Connection,
}

impl Connection {
    /// Establishes a QUIC/mTLS connection to the Manager and performs the
    /// initial handshake.
    ///
    /// Uses [`ConnectionBuilder::new()`] as the single entrypoint for
    /// establishing the connection. No direct QUIC wiring is done here.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * The certificate or key is invalid
    /// * The CA certificates cannot be parsed
    /// * The QUIC connection cannot be established
    /// * The handshake with the Manager fails
    pub async fn connect(
        server_name: &str,
        server_addr: SocketAddr,
        cert_pem: &[u8],
        key_pem: &[u8],
        ca_certs_pem: &[u8],
    ) -> Result<Self> {
        let mut builder = ConnectionBuilder::new(
            server_name,
            server_addr,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            PROTOCOL_VERSION,
            review_protocol::types::Status::Ready,
            cert_pem,
            key_pem,
        )
        .context("failed to create connection builder")?;

        builder
            .add_root_certs(&mut std::io::Cursor::new(ca_certs_pem))
            .context("failed to add root certificates")?;

        let conn = builder
            .connect()
            .await
            .context("failed to connect to Manager")?;

        tracing::info!("Connected to Manager at {server_addr}");
        Ok(Self { inner: conn })
    }

    /// Returns the remote address of the Manager.
    #[must_use]
    pub fn remote_addr(&self) -> SocketAddr {
        self.inner.remote_addr()
    }

    /// Runs the main message processing loop, receiving requests from the
    /// Manager and delegating them to the request handler.
    ///
    /// The loop exits when the connection is closed by the Manager or an
    /// unrecoverable error occurs.
    ///
    /// # Errors
    ///
    /// Returns an error if request handling fails fatally.
    pub async fn run(self) -> Result<()> {
        tracing::info!("Starting message processing loop");
        let mut handler = RequestHandler;
        loop {
            let (mut send, mut recv) = match self.inner.accept_bi().await {
                Ok(streams) => streams,
                Err(e) => {
                    tracing::info!("Connection to Manager closed: {e}");
                    return Ok(());
                }
            };

            if let Err(e) = handle_request(&mut handler, &mut send, &mut recv).await {
                tracing::error!("Request handling failed: {e}");
            }
        }
    }
}

/// Dispatches an incoming request from the Manager to the appropriate handler.
///
/// Receives the request code and payload from the stream, dispatches to the
/// corresponding [`RequestHandler`] method, and sends the response.
///
/// # Errors
///
/// Returns an error if the request cannot be read or the response cannot be
/// sent.
async fn handle_request(
    handler: &mut RequestHandler,
    send: &mut quinn::SendStream,
    recv: &mut quinn::RecvStream,
) -> Result<()> {
    tracing::info!("Received request from Manager, dispatching to handler");
    review_protocol::request::handle(handler, send, recv)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Scaffolding request handler for review-protocol messages.
///
/// All explicitly listed request handlers fail with `unimplemented!()`.
/// Other request types use the default [`review_protocol::request::Handler`]
/// implementations, which return `Err("not supported")`.
///
/// Future issues will implement actual handler logic for each request type,
/// including privileged operations like reboot/shutdown and system metrics
/// collection.
struct RequestHandler;

#[async_trait::async_trait]
impl review_protocol::request::Handler for RequestHandler {
    // Future: implement Reboot handler for system restart
    async fn reboot(&mut self) -> Result<(), String> {
        unimplemented!("Reboot handler not yet implemented")
    }

    // Future: implement Shutdown handler for graceful system shutdown
    async fn shutdown(&mut self) -> Result<(), String> {
        unimplemented!("Shutdown handler not yet implemented")
    }

    // Future: implement ResourceUsage handler to report system metrics
    async fn resource_usage(
        &mut self,
    ) -> Result<(String, review_protocol::types::ResourceUsage), String> {
        unimplemented!("ResourceUsage handler not yet implemented")
    }

    // Future: implement ProcessList handler to list running processes
    async fn process_list(&mut self) -> Result<Vec<review_protocol::types::Process>, String> {
        unimplemented!("ProcessList handler not yet implemented")
    }

    // All other RequestCodes use the default Handler implementations,
    // which return Err("not supported") as an explicit failure.
}
