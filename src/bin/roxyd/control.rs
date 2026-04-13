//! Connection lifecycle and request dispatch for the review-protocol client.
//!
//! This module owns the full connection lifecycle for communicating with the
//! Manager: connect, run loop, stream accept, reconnect, and dispatch entry.
//! All request handling is delegated to the [`handlers`] module.

use std::time::Duration;

use anyhow::{Context, Result};
use review_protocol::client::ConnectionBuilder;
use review_protocol::types::node::{
    NodeHostnameRequest, NodeHostnameResponse, NodeLoggingRequest, NodeLoggingResponse,
    NodeNetworkInterfaceRequest, NodeNetworkInterfaceResponse, NodeObservationRequest,
    NodeObservationResponse, NodePowerRequest, NodePowerResponse, NodeRemoteAccessRequest,
    NodeRemoteAccessResponse, NodeServiceRequest, NodeServiceResponse, NodeTimeSyncRequest,
    NodeTimeSyncResponse, NodeVersionRequest, NodeVersionResponse,
};
use tokio::sync::watch;

use super::{handlers, settings::Settings};

/// The review-protocol version required by this client.
const REQUIRED_REVIEW_VERSION: &str = "0.47.0";

/// A cloneable handle that coordinates graceful shutdown across tasks.
///
/// Call [`trigger`](Self::trigger) to initiate shutdown, and
/// [`recv`](Self::recv) to obtain a receiver that resolves when
/// shutdown has been requested.
#[derive(Clone)]
pub(crate) struct Shutdown {
    tx: watch::Sender<bool>,
}

impl Shutdown {
    /// Creates a new shutdown coordinator.
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self { tx }
    }

    /// Signals all receivers that shutdown has been requested.
    ///
    /// This method is idempotent: calling it more than once is safe.
    pub fn trigger(&self) {
        let _ = self.tx.send(true);
    }

    /// Returns a receiver that completes when shutdown is requested.
    pub fn recv(&self) -> ShutdownRecv {
        ShutdownRecv {
            rx: self.tx.subscribe(),
        }
    }
}

/// A receiver that resolves when shutdown has been requested.
pub(crate) struct ShutdownRecv {
    rx: watch::Receiver<bool>,
}

impl ShutdownRecv {
    /// Waits until shutdown is requested.
    pub async fn wait(&mut self) {
        // If already signalled, return immediately.
        if *self.rx.borrow() {
            return;
        }
        // Wait for the value to change to true.
        let _ = self.rx.wait_for(|&v| v).await;
    }
}

/// A connection to the Manager via review-protocol.
///
/// Owns the underlying QUIC/mTLS connection and the parameters needed to
/// reconnect after a connection loss.
pub(crate) struct Connection {
    builder: ConnectionBuilder,
}

impl Connection {
    #[cfg(not(test))]
    const RECONNECTION_DELAY: Duration = Duration::from_secs(10);
    #[cfg(test)]
    const RECONNECTION_DELAY: Duration = Duration::from_millis(10);

    /// Creates a new `Connection` with a reusable TLS-configured builder.
    ///
    /// Loads the client certificate, private key, and CA certificates from the
    /// configured paths and uses them to prepare a reusable connection builder.
    ///
    /// # Errors
    ///
    /// Returns an error if the configured TLS files cannot be read or if the
    /// connection builder cannot be initialized from the configured credentials.
    pub fn new(settings: &Settings) -> Result<Self> {
        let (cert_pem, key_pem, ca_certs_pem) = settings.load_tls_materials()?;
        let mut builder = ConnectionBuilder::new(
            &settings.server_name,
            settings.server_addr,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            REQUIRED_REVIEW_VERSION,
            review_protocol::types::Status::Ready,
            &cert_pem,
            &key_pem,
        )
        .context("failed to create connection builder")?;

        for pem in &ca_certs_pem {
            let mut reader = std::io::Cursor::new(pem);
            builder
                .add_root_certs(&mut reader)
                .context("failed to add root certificates")?;
        }

        Ok(Self { builder })
    }

    /// Establishes a QUIC/mTLS connection to the Manager and performs the
    /// initial handshake.
    ///
    /// # Errors
    ///
    /// Returns an error if the QUIC connection cannot be established or the
    /// handshake with the Manager fails.
    async fn connect(&self) -> Result<review_protocol::client::Connection> {
        let conn = self
            .builder
            .connect()
            .await
            .context("failed to connect to Manager")?;

        tracing::info!("Connected to Manager at {}", conn.remote_addr());
        Ok(conn)
    }

    /// Runs the main connection loop with automatic reconnection.
    ///
    /// Connects to the Manager, then accepts incoming bidirectional streams
    /// and dispatches each to the request handler. When the connection is
    /// closed by the Manager or an error occurs, it reconnects automatically.
    ///
    /// The loop exits cleanly when `shutdown` is signalled.
    ///
    /// # Errors
    ///
    /// Returns an error if the initial connection cannot be established.
    /// Subsequent reconnection failures are logged and retried.
    pub async fn run(&self, mut shutdown: ShutdownRecv) -> Result<()> {
        let mut inner = self.connect().await?;
        loop {
            tracing::info!("Starting message processing loop");
            let shutdown_requested = loop {
                tokio::select! {
                    result = inner.accept_bi() => {
                        match result {
                            Ok((mut send, mut recv)) => {
                                if let Err(e) = dispatch(&mut send, &mut recv).await {
                                    tracing::error!("Request handling failed: {e}");
                                }
                            }
                            Err(e) => {
                                tracing::info!("Connection to Manager closed: {e}");
                                break false;
                            }
                        }
                    }
                    () = shutdown.wait() => {
                        break true;
                    }
                }
            };

            if shutdown_requested {
                return Ok(());
            }

            tracing::info!("Reconnecting to Manager...");
            tokio::select! {
                conn = self.reconnect_loop() => {
                    inner = conn;
                }
                () = shutdown.wait() => {
                    return Ok(());
                }
            }
        }
    }

    async fn reconnect_loop(&self) -> review_protocol::client::Connection {
        loop {
            match self.connect().await {
                Ok(conn) => return conn,
                Err(e) => {
                    tracing::error!("Failed to reconnect to Manager: {e:#}");
                    tokio::time::sleep(Self::RECONNECTION_DELAY).await;
                }
            }
        }
    }
}

/// Dispatch entry: receives incoming requests from the Manager and routes
/// them to the appropriate handler.
///
/// The local dispatch contract is implemented via [`RequestHandler`], which
/// maps the review-protocol handler callbacks to grouped roxyd handlers
/// under [`handlers`] (e.g. `node_power` → [`handlers::power`],
/// `node_observation` → [`handlers::observation`]).
///
/// Legacy flat methods (`reboot`, `shutdown`, `process_list`,
/// `resource_usage`) are compatibility adapters that delegate through the
/// grouped handlers.
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

/// Request handler that maps review-protocol requests into roxyd handlers.
///
/// roxyd is assumed to run with sudo privilege from startup; there is no
/// privileged vs non-privileged split.
///
/// The grouped `node_*` methods are the canonical internal interface and
/// delegate to the corresponding handler module under [`handlers`].
/// The legacy flat methods (`reboot`, `shutdown`, `process_list`,
/// `resource_usage`) are temporary protocol-compatibility adapters that
/// route through the grouped handlers.
struct RequestHandler;

#[async_trait::async_trait]
impl review_protocol::request::Handler for RequestHandler {
    // -- Grouped node handlers (canonical) --------------------------------

    async fn node_service(
        &mut self,
        req: NodeServiceRequest,
    ) -> Result<NodeServiceResponse, String> {
        tracing::info!(handler_group = "node_service", request = %req.service_id(), "Dispatching request");
        handlers::service::handle(req).await
    }

    async fn node_hostname(
        &mut self,
        req: NodeHostnameRequest,
    ) -> Result<NodeHostnameResponse, String> {
        tracing::info!(handler_group = "node_hostname", request = %req.service_id(), "Dispatching request");
        handlers::hostname::handle(req).await
    }

    async fn node_time_sync(
        &mut self,
        req: NodeTimeSyncRequest,
    ) -> Result<NodeTimeSyncResponse, String> {
        tracing::info!(handler_group = "node_time_sync", request = %req.service_id(), "Dispatching request");
        handlers::time_sync::handle(req).await
    }

    async fn node_logging(
        &mut self,
        req: NodeLoggingRequest,
    ) -> Result<NodeLoggingResponse, String> {
        tracing::info!(handler_group = "node_logging", request = %req.service_id(), "Dispatching request");
        handlers::logging::handle(req).await
    }

    async fn node_remote_access(
        &mut self,
        req: NodeRemoteAccessRequest,
    ) -> Result<NodeRemoteAccessResponse, String> {
        tracing::info!(handler_group = "node_remote_access", request = %req.service_id(), "Dispatching request");
        handlers::remote_access::handle(req).await
    }

    async fn node_power(&mut self, req: NodePowerRequest) -> Result<NodePowerResponse, String> {
        tracing::info!(handler_group = "node_power", request = %req.service_id(), "Dispatching request");
        handlers::power::handle(req).await
    }

    async fn node_observation(
        &mut self,
        req: NodeObservationRequest,
    ) -> Result<NodeObservationResponse, String> {
        tracing::info!(handler_group = "node_observation", request = %req.service_id(), "Dispatching request");
        handlers::observation::handle(req).await
    }

    async fn node_network_interface(
        &mut self,
        req: NodeNetworkInterfaceRequest,
    ) -> Result<NodeNetworkInterfaceResponse, String> {
        tracing::info!(handler_group = "node_network_interface", request = %req.service_id(), "Dispatching request");
        handlers::network_interface::handle(req).await
    }

    async fn node_version(
        &mut self,
        req: NodeVersionRequest,
    ) -> Result<NodeVersionResponse, String> {
        tracing::info!(handler_group = "node_version", request = %req.service_id(), "Dispatching request");
        handlers::version::handle(req).await
    }

    // -- Legacy flat methods (compatibility adapters) ----------------------
    //
    // These delegate through the grouped node handlers so that the new
    // handler modules are the single source of truth. They will be removed
    // once the Manager switches fully to the `node_*` wire format.

    async fn reboot(&mut self) -> Result<(), String> {
        tracing::info!(
            legacy_method = "reboot",
            "Dispatching legacy request via node_power"
        );
        self.node_power(NodePowerRequest::Reboot).await.map(|_| ())
    }

    async fn shutdown(&mut self) -> Result<(), String> {
        tracing::info!(
            legacy_method = "shutdown",
            "Dispatching legacy request via node_power"
        );
        self.node_power(NodePowerRequest::Shutdown)
            .await
            .map(|_| ())
    }

    async fn resource_usage(
        &mut self,
    ) -> Result<(String, review_protocol::types::ResourceUsage), String> {
        tracing::info!(
            legacy_method = "resource_usage",
            "Dispatching legacy request via node_observation"
        );
        match self
            .node_observation(NodeObservationRequest::ResourceUsage)
            .await?
        {
            NodeObservationResponse::ResourceUsage {
                hostname,
                resource_usage,
            } => Ok((hostname, resource_usage)),
            other => Err(format!("unexpected observation response: {other:?}")),
        }
    }

    async fn process_list(&mut self) -> Result<Vec<review_protocol::types::Process>, String> {
        tracing::info!(
            legacy_method = "process_list",
            "Dispatching legacy request via node_observation"
        );
        match self
            .node_observation(NodeObservationRequest::ProcessList)
            .await?
        {
            NodeObservationResponse::ProcessList { processes } => Ok(processes),
            other => Err(format!("unexpected observation response: {other:?}")),
        }
    }

    // -- Unsupported flat methods -----------------------------------------

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
    use std::{fs, net::SocketAddr, sync::Arc};

    use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use tempfile::{TempDir, tempdir};

    use super::*;
    use crate::settings::{Config, Settings};

    const TEST_PROTOCOL_VERSION: &str = "0.47.0";
    const TEST_BIND_ADDR: &str = "127.0.0.1:0";

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
    fn build_mock_manager_endpoint(certs: &TestCerts, addr: SocketAddr) -> quinn::Endpoint {
        build_mock_manager_endpoint_with_chain(
            certs,
            addr,
            vec![certs.server_cert_der.clone(), certs.inter_cert_der.clone()],
        )
    }

    /// Creates a QUIC server endpoint with a custom server certificate chain.
    fn build_mock_manager_endpoint_with_chain(
        certs: &TestCerts,
        addr: SocketAddr,
        server_cert_chain: Vec<CertificateDer<'static>>,
    ) -> quinn::Endpoint {
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

        let server_tls = rustls::ServerConfig::builder()
            .with_client_cert_verifier(client_verifier)
            .with_single_cert(server_cert_chain, certs.server_key_der.clone_key())
            .expect("server TLS config");

        let quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls)
            .expect("QUIC server config");
        let server_config = quinn::ServerConfig::with_crypto(Arc::new(quic_config));
        quinn::Endpoint::server(server_config, addr).expect("server endpoint")
    }

    fn build_test_settings(
        server_name: &str,
        server_addr: SocketAddr,
        cert_pem: &[u8],
        key_pem: &[u8],
        ca_certs_pem: &[&[u8]],
    ) -> (TempDir, Settings) {
        let dir = tempdir().expect("temp dir");

        let cert_path = dir.path().join("cert.pem");
        let key_path = dir.path().join("key.pem");
        fs::write(&cert_path, cert_pem).expect("write cert");
        fs::write(&key_path, key_pem).expect("write key");

        let mut ca_paths = Vec::with_capacity(ca_certs_pem.len());
        for (idx, pem) in ca_certs_pem.iter().enumerate() {
            let path = dir.path().join(format!("ca-{idx}.pem"));
            fs::write(&path, pem).expect("write ca cert");
            ca_paths.push(path);
        }

        let settings = Settings {
            server_name: server_name.to_string(),
            server_addr,
            cert: cert_path,
            key: key_path,
            ca_certs: ca_paths,
            config: Config { log_path: None },
        };

        (dir, settings)
    }

    /// Sets up a mock Manager and client, performs the handshake, and returns
    /// the client connection, the server connection, and the endpoint (which
    /// must be kept alive).
    async fn setup_test_connection() -> (
        review_protocol::client::Connection,
        review_protocol::server::Connection,
        quinn::Endpoint,
    ) {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        let endpoint = build_mock_manager_endpoint(&certs, addr);
        let server_addr = endpoint.local_addr().expect("server addr");
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[
                certs.root_cert_pem.as_bytes(),
                certs.inter_cert_pem.as_bytes(),
            ],
        );

        let conn = Connection::new(&settings).expect("client connection config");

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
            conn.connect()
        );

        (client_conn.expect("client connect"), server_conn, endpoint)
    }

    // -- Test: QUIC/mTLS connection + handshake with CA bundle --------

    #[tokio::test]
    async fn connect_and_handshake_with_mock_manager() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        let endpoint = build_mock_manager_endpoint(&certs, addr);
        let server_addr = endpoint.local_addr().expect("server addr");
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[
                certs.root_cert_pem.as_bytes(),
                certs.inter_cert_pem.as_bytes(),
            ],
        );

        let conn = Connection::new(&settings).expect("client connection config");

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
            conn.connect()
        );

        assert_eq!(agent_info.app_name, env!("CARGO_PKG_NAME"));
        let inner = conn_result.expect("client connection");
        assert_eq!(inner.remote_addr(), server_addr);
    }

    #[tokio::test]
    async fn connect_and_handshake_with_combined_ca_bundle() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        let endpoint = build_mock_manager_endpoint(&certs, addr);
        let server_addr = endpoint.local_addr().expect("server addr");
        let combined_ca_bundle = format!("{}{}", certs.root_cert_pem, certs.inter_cert_pem);
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[combined_ca_bundle.as_bytes()],
        );

        let conn = Connection::new(&settings).expect("client connection config");

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
            conn.connect()
        );

        assert_eq!(agent_info.app_name, env!("CARGO_PKG_NAME"));
        let inner = conn_result.expect("client connection");
        assert_eq!(inner.remote_addr(), server_addr);
    }

    // -- Tests: chain building with root-only trust store ---------------

    /// Client trusts only the root CA. The server sends `[server, intermediate]`,
    /// so the client can build the full chain and the handshake succeeds.
    #[tokio::test]
    async fn handshake_succeeds_with_root_only_trust_when_peer_sends_intermediate() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        // Server sends full chain: server cert + intermediate cert
        let endpoint = build_mock_manager_endpoint_with_chain(
            &certs,
            addr,
            vec![certs.server_cert_der.clone(), certs.inter_cert_der.clone()],
        );
        let server_addr = endpoint.local_addr().expect("server addr");
        // Client trusts only the root CA
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[certs.root_cert_pem.as_bytes()],
        );

        let conn = Connection::new(&settings).expect("client connection config");

        let ((_agent_info, _quinn_conn), conn_result) = tokio::join!(
            async {
                let incoming = endpoint.accept().await.expect("accept");
                let quinn_conn = incoming.await.expect("incoming conn");
                let peer_addr = quinn_conn.remote_address();
                let version_req = format!(">={TEST_PROTOCOL_VERSION}");
                let agent_info = review_protocol::server::handshake(
                    &quinn_conn,
                    peer_addr,
                    &version_req,
                    TEST_PROTOCOL_VERSION,
                )
                .await
                .expect("server handshake");
                (agent_info, quinn_conn)
            },
            conn.connect()
        );

        conn_result.expect("handshake should succeed when peer sends intermediate");
    }

    /// Client trusts only the root CA. The server sends `[server]` without the
    /// intermediate, so the client cannot build the chain and the connection
    /// fails.
    #[tokio::test]
    async fn handshake_fails_with_root_only_trust_when_peer_omits_intermediate() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        // Server sends only the server cert — no intermediate
        let endpoint = build_mock_manager_endpoint_with_chain(
            &certs,
            addr,
            vec![certs.server_cert_der.clone()],
        );
        let server_addr = endpoint.local_addr().expect("server addr");
        // Client trusts only the root CA
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[certs.root_cert_pem.as_bytes()],
        );

        let conn = Connection::new(&settings).expect("client connection config");

        let conn_result = tokio::time::timeout(Duration::from_secs(5), async {
            let (_server_result, client_result) = tokio::join!(
                async {
                    // Server side: accept and attempt handshake (may fail)
                    let incoming = endpoint.accept().await.expect("accept");
                    incoming.await
                },
                conn.connect()
            );
            client_result
        })
        .await
        .expect("should not hang");

        assert!(
            conn_result.is_err(),
            "handshake should fail when peer omits intermediate"
        );
    }

    // -- Test: request/response flow after handshake (ping/echo) ------

    #[tokio::test]
    async fn handshake_and_request_response_flow() {
        let (inner, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut send, mut recv)) = inner.accept_bi().await else {
                    return Ok::<(), anyhow::Error>(());
                };
                if let Err(e) = dispatch(&mut send, &mut recv).await {
                    tracing::error!("Request handling failed: {e}");
                }
            }
        });

        let ping_result = server.send_ping().await;
        assert!(ping_result.is_ok(), "ping should succeed: {ping_result:?}");

        drop(server);
        let task_result = task.await.expect("task should not panic");
        assert!(task_result.is_ok());
    }

    // -- Tests: RequestCode dispatch over live connection --------------

    #[tokio::test]
    async fn dispatch_reboot_over_live_connection() {
        let (inner, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut send, mut recv)) = inner.accept_bi().await else {
                    return Ok::<(), anyhow::Error>(());
                };
                if let Err(e) = dispatch(&mut send, &mut recv).await {
                    tracing::error!("Request handling failed: {e}");
                }
            }
        });

        let result = server.send_reboot_cmd().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_shutdown_over_live_connection() {
        let (inner, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut send, mut recv)) = inner.accept_bi().await else {
                    return Ok::<(), anyhow::Error>(());
                };
                if let Err(e) = dispatch(&mut send, &mut recv).await {
                    tracing::error!("Request handling failed: {e}");
                }
            }
        });

        let result = server.send_shutdown_cmd().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_resource_usage_over_live_connection() {
        let (inner, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut send, mut recv)) = inner.accept_bi().await else {
                    return Ok::<(), anyhow::Error>(());
                };
                if let Err(e) = dispatch(&mut send, &mut recv).await {
                    tracing::error!("Request handling failed: {e}");
                }
            }
        });

        let result = server.get_resource_usage().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn dispatch_process_list_over_live_connection() {
        let (inner, server, _endpoint) = setup_test_connection().await;
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut send, mut recv)) = inner.accept_bi().await else {
                    return Ok::<(), anyhow::Error>(());
                };
                if let Err(e) = dispatch(&mut send, &mut recv).await {
                    tracing::error!("Request handling failed: {e}");
                }
            }
        });

        let result = server.get_process_list().await;
        assert!(result.is_err(), "should fail: handler is unimplemented");

        let task_err = task.await.expect_err("task should have panicked");
        assert!(task_err.is_panic());
    }

    #[tokio::test]
    async fn run_reconnects_after_connection_close() {
        use tokio::sync::oneshot;

        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let bind_addr: SocketAddr = TEST_BIND_ADDR.parse().expect("bind addr");
        let endpoint = build_mock_manager_endpoint(&certs, bind_addr);
        let addr = endpoint.local_addr().expect("server addr");
        let (_dir, settings) = build_test_settings(
            "localhost",
            addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[
                certs.root_cert_pem.as_bytes(),
                certs.inter_cert_pem.as_bytes(),
            ],
        );

        let conn = Connection::new(&settings).expect("client connection config");
        let shutdown = Shutdown::new();

        let (reconnected_tx, reconnected_rx) = oneshot::channel();

        let server_task = tokio::spawn(async move {
            let incoming = endpoint.accept().await.expect("accept first connection");
            let quinn_conn = incoming.await.expect("first incoming connection");
            let peer_addr = quinn_conn.remote_address();
            let version_req = format!(">={TEST_PROTOCOL_VERSION}");
            let _agent_info = review_protocol::server::handshake(
                &quinn_conn,
                peer_addr,
                &version_req,
                TEST_PROTOCOL_VERSION,
            )
            .await
            .expect("first server handshake");

            let server_conn = review_protocol::server::Connection::from_quinn(quinn_conn.clone());
            server_conn
                .send_ping()
                .await
                .expect("send ping before close");
            quinn_conn.close(0u32.into(), b"test reconnect");
            drop(server_conn);
            drop(quinn_conn);

            let incoming = endpoint.accept().await.expect("accept second connection");
            let quinn_conn = incoming.await.expect("second incoming connection");
            let peer_addr = quinn_conn.remote_address();
            let _agent_info = review_protocol::server::handshake(
                &quinn_conn,
                peer_addr,
                &version_req,
                TEST_PROTOCOL_VERSION,
            )
            .await
            .expect("second server handshake");

            let _ = reconnected_tx.send(());
            std::future::pending::<()>().await;
        });

        let shutdown_recv = shutdown.recv();
        let run_task = tokio::spawn(async move { conn.run(shutdown_recv).await });

        tokio::time::timeout(Duration::from_secs(1), reconnected_rx)
            .await
            .expect("run should reconnect after the connection is closed")
            .expect("reconnect notification should be sent");

        shutdown.trigger();
        let run_result = tokio::time::timeout(Duration::from_secs(1), run_task)
            .await
            .expect("run should exit after shutdown")
            .expect("run task should not panic");
        assert!(run_result.is_ok(), "run should return Ok on clean shutdown");

        server_task.abort();
        let server_abort = server_task
            .await
            .expect_err("server task should be aborted");
        assert!(server_abort.is_cancelled());
    }

    #[tokio::test]
    async fn run_exits_cleanly_on_shutdown_during_accept() {
        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        let endpoint = build_mock_manager_endpoint(&certs, addr);
        let server_addr = endpoint.local_addr().expect("server addr");
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[
                certs.root_cert_pem.as_bytes(),
                certs.inter_cert_pem.as_bytes(),
            ],
        );

        let conn = Connection::new(&settings).expect("client connection config");
        let shutdown = Shutdown::new();
        let shutdown_recv = shutdown.recv();

        // Server accepts and completes handshake, then keeps connection
        // alive so the client blocks on accept_bi.
        let server_task = tokio::spawn(async move {
            let incoming = endpoint.accept().await.expect("accept");
            let quinn_conn = incoming.await.expect("incoming conn");
            let peer_addr = quinn_conn.remote_address();
            let version_req = format!(">={TEST_PROTOCOL_VERSION}");
            let _agent_info = review_protocol::server::handshake(
                &quinn_conn,
                peer_addr,
                &version_req,
                TEST_PROTOCOL_VERSION,
            )
            .await
            .expect("server handshake");
            std::future::pending::<()>().await;
        });

        let run_task = tokio::spawn(async move { conn.run(shutdown_recv).await });

        // Give the client time to enter the accept loop.
        tokio::time::sleep(Duration::from_millis(50)).await;

        shutdown.trigger();

        let result = tokio::time::timeout(Duration::from_secs(2), run_task)
            .await
            .expect("run should exit after shutdown")
            .expect("run task should not panic");
        assert!(result.is_ok(), "run should return Ok on clean shutdown");

        server_task.abort();
    }

    #[tokio::test]
    async fn run_exits_cleanly_on_shutdown_during_reconnect() {
        use tokio::sync::oneshot;

        let _ = rustls::crypto::ring::default_provider().install_default();

        let certs = generate_certs();
        let addr: SocketAddr = TEST_BIND_ADDR.parse().expect("addr");
        let endpoint = build_mock_manager_endpoint(&certs, addr);
        let server_addr = endpoint.local_addr().expect("server addr");
        let (_dir, settings) = build_test_settings(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            &[
                certs.root_cert_pem.as_bytes(),
                certs.inter_cert_pem.as_bytes(),
            ],
        );

        let conn = Connection::new(&settings).expect("client connection config");
        let shutdown = Shutdown::new();
        let shutdown_recv = shutdown.recv();

        // Signal when the server has closed the first connection so we
        // know the client is about to enter the reconnect loop.
        let (closed_tx, closed_rx) = oneshot::channel();

        // Server accepts first connection, handshakes, sends a ping to
        // ensure the client is fully connected, then closes it.
        let server_task = tokio::spawn(async move {
            let incoming = endpoint.accept().await.expect("accept");
            let quinn_conn = incoming.await.expect("incoming conn");
            let peer_addr = quinn_conn.remote_address();
            let version_req = format!(">={TEST_PROTOCOL_VERSION}");
            let _agent_info = review_protocol::server::handshake(
                &quinn_conn,
                peer_addr,
                &version_req,
                TEST_PROTOCOL_VERSION,
            )
            .await
            .expect("server handshake");

            let server_conn = review_protocol::server::Connection::from_quinn(quinn_conn.clone());
            server_conn
                .send_ping()
                .await
                .expect("send ping before close");
            quinn_conn.close(0u32.into(), b"force close");
            drop(server_conn);
            let _ = closed_tx.send(());
            // Keep endpoint alive so the reconnect loop keeps retrying.
            std::future::pending::<()>().await;
        });

        let run_task = tokio::spawn(async move { conn.run(shutdown_recv).await });

        // Wait until the server has closed the connection.
        closed_rx.await.expect("closed notification");
        // Give the client time to detect the close and enter reconnect.
        tokio::time::sleep(Duration::from_millis(50)).await;

        shutdown.trigger();

        let result = tokio::time::timeout(Duration::from_secs(2), run_task)
            .await
            .expect("run should exit after shutdown")
            .expect("run task should not panic");
        assert!(result.is_ok(), "run should return Ok on clean shutdown");

        server_task.abort();
    }
}
