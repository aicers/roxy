//! End-to-end test for the review-protocol client connection skeleton.
//!
//! Spins up a minimal mock Manager (review-protocol server) and verifies
//! QUIC/mTLS connection establishment and handshake completion.

use std::net::SocketAddr;
use std::sync::Arc;

use rcgen::{BasicConstraints, CertificateParams, DnType, IsCa, Issuer, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};

/// Protocol version used by both the mock Manager and the client.
const PROTOCOL_VERSION: &str = "0.16.0";

/// Certificates generated for a single test run.
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

/// Generates a certificate chain for testing:
/// root CA -> intermediate CA -> server cert / client cert.
///
/// The CA bundle (root + intermediate) is provided for trust configuration,
/// mirroring the real Manager's TLS setup.
fn generate_certs() -> TestCerts {
    // Root CA
    let root_key = KeyPair::generate().expect("root key generation");
    let mut root_params = CertificateParams::default();
    root_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    root_params
        .distinguished_name
        .push(DnType::CommonName, "Test Root CA");
    let root_cert = root_params
        .self_signed(&root_key)
        .expect("root cert self-sign");
    let root_issuer = Issuer::from_params(&root_params, &root_key);

    // Intermediate CA (signed by root)
    let inter_key = KeyPair::generate().expect("intermediate key generation");
    let mut inter_params = CertificateParams::default();
    inter_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    inter_params
        .distinguished_name
        .push(DnType::CommonName, "Test Intermediate CA");
    let inter_cert = inter_params
        .signed_by(&inter_key, &root_issuer)
        .expect("intermediate cert signing");
    let inter_issuer = Issuer::from_params(&inter_params, &inter_key);

    // Server certificate (signed by intermediate)
    let server_key = KeyPair::generate().expect("server key generation");
    let server_params =
        CertificateParams::new(vec!["localhost".to_string()]).expect("server cert params");
    let server_cert = server_params
        .signed_by(&server_key, &inter_issuer)
        .expect("server cert signing");

    // Client certificate (signed by intermediate)
    let client_key = KeyPair::generate().expect("client key generation");
    let client_params =
        CertificateParams::new(vec!["test-client".to_string()]).expect("client cert params");
    let client_cert = client_params
        .signed_by(&client_key, &inter_issuer)
        .expect("client cert signing");

    TestCerts {
        root_cert_pem: root_cert.pem(),
        inter_cert_pem: inter_cert.pem(),
        server_cert_der: server_cert.der().clone(),
        inter_cert_der: inter_cert.der().clone(),
        root_cert_der: root_cert.der().clone(),
        server_key_der: PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(server_key.serialize_der())),
        client_cert_pem: client_cert.pem(),
        client_key_pem: client_key.serialize_pem(),
    }
}

/// Creates a quinn server endpoint configured for mTLS with the test certs.
fn build_mock_manager_endpoint(certs: &TestCerts) -> quinn::Endpoint {
    // Build root cert store for client authentication
    let mut root_store = rustls::RootCertStore::empty();
    root_store
        .add(certs.root_cert_der.clone())
        .expect("add root cert");
    root_store
        .add(certs.inter_cert_der.clone())
        .expect("add intermediate cert");

    let client_verifier = rustls::server::WebPkiClientVerifier::builder(Arc::new(root_store))
        .build()
        .expect("build client verifier");

    // Server cert chain: server cert + intermediate cert
    let server_cert_chain = vec![certs.server_cert_der.clone(), certs.inter_cert_der.clone()];

    let server_tls_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(client_verifier)
        .with_single_cert(server_cert_chain, certs.server_key_der.clone_key())
        .expect("build server TLS config");

    let server_quic_config = quinn::crypto::rustls::QuicServerConfig::try_from(server_tls_config)
        .expect("build QUIC server config");

    let server_config = quinn::ServerConfig::with_crypto(Arc::new(server_quic_config));

    // Bind to ephemeral port on localhost
    let addr: SocketAddr = "127.0.0.1:0".parse().expect("parse addr");
    quinn::Endpoint::server(server_config, addr).expect("create server endpoint")
}

/// Runs a minimal mock Manager that performs only the handshake.
/// No `RequestCode` business logic is implemented.
///
/// Returns both the agent info from the handshake and the server-side
/// connection. The connection must be kept alive until the client has
/// finished its handshake processing; dropping it sends `CONNECTION_CLOSE`.
async fn run_mock_manager(
    endpoint: &quinn::Endpoint,
) -> (review_protocol::AgentInfo, quinn::Connection) {
    let incoming = endpoint.accept().await.expect("accept incoming connection");
    let conn = incoming.await.expect("complete incoming connection");
    let addr = conn.remote_address();

    // Perform the review-protocol handshake
    let version_req = format!(">={PROTOCOL_VERSION}");
    let agent_info =
        review_protocol::server::handshake(&conn, addr, &version_req, PROTOCOL_VERSION)
            .await
            .expect("server handshake");

    // Return the connection to keep it alive until the caller drops it
    (agent_info, conn)
}

/// Verifies QUIC/mTLS connection establishment and handshake completion
/// between a review-protocol client and a minimal mock Manager.
#[tokio::test]
async fn connect_and_handshake_with_mock_manager() {
    // Install the ring crypto provider since both ring and aws-lc-rs
    // features may be enabled via transitive dependencies.
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("install ring crypto provider");

    let certs = generate_certs();
    let endpoint = build_mock_manager_endpoint(&certs);
    let server_addr = endpoint.local_addr().expect("get server local addr");

    // Build CA bundle (root + intermediate) for client trust
    let ca_bundle = format!("{}{}", certs.root_cert_pem, certs.inter_cert_pem);

    // Run mock Manager and client concurrently. Using join! keeps the
    // endpoint alive until both sides have completed the handshake.
    let ((agent_info, _server_conn), conn_result) = tokio::join!(
        run_mock_manager(&endpoint),
        roxy::review_client::Connection::connect(
            "localhost",
            server_addr,
            certs.client_cert_pem.as_bytes(),
            certs.client_key_pem.as_bytes(),
            ca_bundle.as_bytes(),
        )
    );

    // Verify server-side handshake received correct app info
    assert_eq!(agent_info.app_name, env!("CARGO_PKG_NAME"));

    let conn = conn_result.expect("client connection and handshake");

    // Verify the connection is alive by checking the remote address
    assert_eq!(conn.remote_addr(), server_addr);
}
