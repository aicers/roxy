//! Connection lifecycle and request dispatch for the review-protocol client.
//!
//! This module owns the full connection lifecycle for communicating with the
//! Manager: connect, reconnect, run loop, stream accept, and dispatch entry.
//! All request handling is delegated to the [`handlers`] module.

mod handlers;

use std::net::SocketAddr;

use anyhow::{Context, Result};
use review_protocol::client::ConnectionBuilder;

/// The protocol version this client supports.
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
    /// establishing the connection.
    ///
    /// # Errors
    ///
    /// Returns an error if the certificate/key is invalid, CA certificates
    /// cannot be parsed, the QUIC connection cannot be established, or the
    /// handshake with the Manager fails.
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

    /// Runs the main message processing loop.
    ///
    /// Accepts incoming bidirectional streams from the Manager and dispatches
    /// each to the request handler. The loop exits when the connection is
    /// closed by the Manager or an unrecoverable error occurs.
    ///
    /// # Errors
    ///
    /// Returns an error if the connection terminates unexpectedly.
    pub async fn run(self) -> Result<()> {
        tracing::info!("Starting message processing loop");
        loop {
            let (mut send, mut recv) = match self.inner.accept_bi().await {
                Ok(streams) => streams,
                Err(e) => {
                    tracing::info!("Connection to Manager closed: {e}");
                    return Ok(());
                }
            };

            if let Err(e) = dispatch(&mut send, &mut recv).await {
                tracing::error!("Request handling failed: {e}");
            }
        }
    }
}

/// Dispatch entry: receives incoming requests from the Manager and routes
/// them to the appropriate handler.
///
/// The local dispatch contract is implemented via [`RequestHandler`], which
/// explicitly matches each [`review_protocol::client::RequestCode`] to its
/// handler:
///
/// - `RequestCode::Reboot` → [`handlers::reboot`]
/// - `RequestCode::Shutdown` → [`handlers::shutdown`]
/// - `RequestCode::ResourceUsage` → [`handlers::resource_usage`]
/// - `RequestCode::ProcessList` → [`handlers::process_list`]
/// - All other codes → explicit `unimplemented!()`
///
/// # Errors
///
/// Returns an error if request reading or response sending fails.
async fn dispatch(send: &mut quinn::SendStream, recv: &mut quinn::RecvStream) -> Result<()> {
    let mut handler = RequestHandler;
    review_protocol::request::handle(&mut handler, send, recv)
        .await
        .map_err(|e| anyhow::anyhow!("{e}"))
}

/// Request handler that explicitly dispatches every [`RequestCode`] arm.
///
/// roxyd is assumed to run with sudo privilege from startup; there is no
/// privileged vs non-privileged split. All four required handlers are
/// scaffolding-only (`unimplemented!()`). All other request codes also
/// fail explicitly with `unimplemented!()` to keep the dispatch contract
/// visible.
struct RequestHandler;

#[async_trait::async_trait]
impl review_protocol::request::Handler for RequestHandler {
    // ── Required handlers (scaffolding) ─────────────────────────────

    async fn reboot(&mut self) -> Result<(), String> {
        handlers::reboot::handle().await
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        handlers::shutdown::handle().await
    }

    async fn resource_usage(
        &mut self,
    ) -> Result<(String, review_protocol::types::ResourceUsage), String> {
        handlers::resource_usage::handle().await
    }

    async fn process_list(&mut self) -> Result<Vec<review_protocol::types::Process>, String> {
        handlers::process_list::handle().await
    }

    // ── Explicit fallback for all other request codes ───────────────

    async fn dns_start(&mut self) -> Result<(), String> {
        unimplemented!("DnsStart not supported by roxyd")
    }

    async fn dns_stop(&mut self) -> Result<(), String> {
        unimplemented!("DnsStop not supported by roxyd")
    }

    async fn forward(&mut self, _target: &str, _msg: &[u8]) -> Result<Vec<u8>, String> {
        unimplemented!("Forward not supported by roxyd")
    }

    #[allow(deprecated)]
    async fn reload_config(&mut self) -> Result<(), String> {
        unimplemented!("ReloadConfig not supported by roxyd")
    }

    async fn update_config(&mut self) -> Result<(), String> {
        unimplemented!("UpdateConfig not supported by roxyd")
    }

    async fn reload_ti(&mut self, _version: &str) -> Result<(), String> {
        unimplemented!("ReloadTi not supported by roxyd")
    }

    async fn tor_exit_node_list(&mut self, _nodes: &[&str]) -> Result<(), String> {
        unimplemented!("TorExitNodeList not supported by roxyd")
    }

    async fn trusted_domain_list(&mut self, _domains: &[&str]) -> Result<(), String> {
        unimplemented!("TrustedDomainList not supported by roxyd")
    }

    async fn sampling_policy_list(
        &mut self,
        _policies: &[review_protocol::types::SamplingPolicy],
    ) -> Result<(), String> {
        unimplemented!("SamplingPolicyList not supported by roxyd")
    }

    async fn update_traffic_filter_rules(
        &mut self,
        _rules: &[review_protocol::types::TrafficFilterRule],
    ) -> Result<(), String> {
        unimplemented!("ReloadFilterRule not supported by roxyd")
    }

    async fn delete_sampling_policy(&mut self, _policy_ids: &[u32]) -> Result<(), String> {
        unimplemented!("DeleteSamplingPolicy not supported by roxyd")
    }

    async fn internal_network_list(
        &mut self,
        _list: review_protocol::types::HostNetworkGroup,
    ) -> Result<(), String> {
        unimplemented!("InternalNetworkList not supported by roxyd")
    }

    async fn allowlist(
        &mut self,
        _list: review_protocol::types::HostNetworkGroup,
    ) -> Result<(), String> {
        unimplemented!("Allowlist not supported by roxyd")
    }

    async fn blocklist(
        &mut self,
        _list: review_protocol::types::HostNetworkGroup,
    ) -> Result<(), String> {
        unimplemented!("Blocklist not supported by roxyd")
    }

    async fn trusted_user_agent_list(&mut self, _list: &[&str]) -> Result<(), String> {
        unimplemented!("TrustedUserAgentList not supported by roxyd")
    }

    async fn update_semi_supervised_models(&mut self, _list: &[u8]) -> Result<(), String> {
        unimplemented!("SemiSupervisedModels not supported by roxyd")
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::Arc;

    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

    use super::*;

    const TEST_PROTOCOL_VERSION: &str = "0.16.0";

    struct TestCerts {
        root_cert_pem: String,
        inter_cert_pem: String,
        server_cert_der: CertificateDer<'static>,
        inter_cert_der: CertificateDer<'static>,
        root_cert_der: CertificateDer<'static>,
        server_key_der: PrivateKeyDer<'static>,
        client_cert_pem: String,
        client_key_pem: String,
    }

    /// Generates a CA chain: root CA -> intermediate CA -> server/client certs.
    fn generate_certs() -> TestCerts {
        let root_key = KeyPair::generate().expect("root key");
        let mut root_params = CertificateParams::default();
        root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        root_params
            .distinguished_name
            .push(DnType::CommonName, "Test Root CA");
        let root_cert = root_params.self_signed(&root_key).expect("root cert");
        let root_issuer = Issuer::from_params(&root_params, &root_key);

        let inter_key = KeyPair::generate().expect("inter key");
        let mut inter_params = CertificateParams::default();
        inter_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        inter_params
            .distinguished_name
            .push(DnType::CommonName, "Test Intermediate CA");
        let inter_cert = inter_params
            .signed_by(&inter_key, &root_issuer)
            .expect("inter cert");
        let inter_issuer = Issuer::from_params(&inter_params, &inter_key);

        let server_key = KeyPair::generate().expect("server key");
        let server_params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("server params");
        let server_cert = server_params
            .signed_by(&server_key, &inter_issuer)
            .expect("server cert");

        let client_key = KeyPair::generate().expect("client key");
        let client_params =
            CertificateParams::new(vec!["test-client".to_string()]).expect("client params");
        let client_cert = client_params
            .signed_by(&client_key, &inter_issuer)
            .expect("client cert");

        TestCerts {
            root_cert_pem: root_cert.pem(),
            inter_cert_pem: inter_cert.pem(),
            server_cert_der: server_cert.der().clone(),
            inter_cert_der: inter_cert.der().clone(),
            root_cert_der: root_cert.der().clone(),
            server_key_der: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(
                server_key.serialize_der(),
            )),
            client_cert_pem: client_cert.pem(),
            client_key_pem: client_key.serialize_pem(),
        }
    }

    /// Creates a QUIC server endpoint configured for mTLS.
    fn build_mock_manager_endpoint(certs: &TestCerts) -> quinn::Endpoint {
        let mut root_store = rustls::RootCertStore::empty();
        root_store
            .add(certs.root_cert_der.clone())
            .expect("add root");
        root_store
            .add(certs.inter_cert_der.clone())
            .expect("add inter");

        let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
            .build()
            .expect("client verifier");

        let server_cert_chain = vec![certs.server_cert_der.clone(), certs.inter_cert_der.clone()];
        let server_tls = rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(server_cert_chain, certs.server_key_der.clone_key())
            .expect("server TLS config");

        let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
            .expect("QUIC server config");
        let server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));
        let addr: SocketAddr = "127.0.0.1:0".parse().expect("addr");
        quinn::Endpoint::server(server_config, addr).expect("server endpoint")
    }

    /// Sets up a mock Manager and client, performs the handshake, and returns
    /// both connections plus the endpoint (which must be kept alive).
    async fn setup_test_connection() -> (
        Connection,
        review_protocol::server::Connection,
        quinn::Endpoint,
    ) {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let endpoint = build_mock_manager_endpoint(&certs);
        let server_addr = endpoint.local_addr().expect("server addr");
        let ca_bundle = format!("{}{}", certs.root_cert_pem, certs.inter_cert_pem);

        let (server_conn, client_conn) = tokio::join!(
            async {
                let incoming = endpoint.accept().await.expect("accept");
                let quinn_conn = incoming.await.expect("incoming conn");
                let addr = quinn_conn.remote_address();
                let version_req = format!(">={TEST_PROTOCOL_VERSION}");
                let _agent_info = review_protocol::server::handshake(
                    &quinn_conn,
                    addr,
                    &version_req,
                    TEST_PROTOCOL_VERSION,
                )
                .await
                .expect("server handshake");
                review_protocol::server::Connection::from_quinn(quinn_conn)
            },
            Connection::connect(
                "localhost",
                server_addr,
                certs.client_cert_pem.as_bytes(),
                certs.client_key_pem.as_bytes(),
                ca_bundle.as_bytes(),
            )
        );

        (client_conn.expect("client connect"), server_conn, endpoint)
    }

    // -- Test: QUIC/mTLS connection + handshake with CA bundle --------

    #[tokio::test]
    async fn connect_and_handshake_with_mock_manager() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let endpoint = build_mock_manager_endpoint(&certs);
        let server_addr = endpoint.local_addr().expect("server addr");
        let ca_bundle = format!("{}{}", certs.root_cert_pem, certs.inter_cert_pem);

        let ((agent_info, _quinn_conn), conn_result) = tokio::join!(
            async {
                let incoming = endpoint.accept().await.expect("accept");
                let quinn_conn = incoming.await.expect("incoming conn");
                let addr = quinn_conn.remote_address();
                let version_req = format!(">={TEST_PROTOCOL_VERSION}");
                let agent_info = review_protocol::server::handshake(
                    &quinn_conn,
                    addr,
                    &version_req,
                    TEST_PROTOCOL_VERSION,
                )
                .await
                .expect("server handshake");
                (agent_info, quinn_conn)
            },
            Connection::connect(
                "localhost",
                server_addr,
                certs.client_cert_pem.as_bytes(),
                certs.client_key_pem.as_bytes(),
                ca_bundle.as_bytes(),
            )
        );

        assert_eq!(agent_info.app_name, env!("CARGO_PKG_NAME"));
        let conn = conn_result.expect("client connection");
        assert_eq!(conn.remote_addr(), server_addr);
    }

    // -- Test: request/response flow after handshake (ping/echo) ------

    #[tokio::test]
    async fn handshake_and_request_response_flow() {
        let (conn, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(conn.run());

        // Verify request/response flow with ping (EchoRequest is handled
        // internally by review_protocol without calling a handler method)
        let ping_result = server.send_ping().await;
        assert!(ping_result.is_ok(), "ping should succeed: {ping_result:?}");

        // Drop server connection to close QUIC, causing client to exit cleanly
        drop(server);
        let task_result = task.await.expect("task should not panic");
        assert!(task_result.is_ok());
    }

    // -- Tests: RequestCode dispatch over live connection --------------

    #[tokio::test]
    async fn dispatch_reboot_over_live_connection() {
        let (conn, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(conn.run());

        let result = server.send_reboot_cmd().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_shutdown_over_live_connection() {
        let (conn, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(conn.run());

        let result = server.send_shutdown_cmd().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_resource_usage_over_live_connection() {
        let (conn, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(conn.run());

        let result = server.get_resource_usage().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_process_list_over_live_connection() {
        let (conn, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(conn.run());

        let result = server.get_process_list().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }
}
