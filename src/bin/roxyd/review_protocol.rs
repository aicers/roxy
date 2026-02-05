//! review-protocol client integration for roxyd.

use std::{fs, net::SocketAddr, time::Duration};

use anyhow::{Context, Result};
use async_trait::async_trait;
use review_protocol::{
    client::{Connection, ConnectionBuilder},
    request::{self, Handler},
    types::{Process, ResourceUsage, Status},
};

use crate::settings::RoxydConfig;

const REQUIRED_REVIEW_VERSION: &str = "0.46.0";
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const MIN_RECONNECT_DELAY: Duration = Duration::from_millis(500);

struct ManagerEndpoint {
    name: String,
    addr: SocketAddr,
}

fn parse_manager_server(manager_server: &str) -> Result<ManagerEndpoint> {
    let (name, addr) = manager_server
        .split_once('@')
        .context("cannot get information of the Manager server")?;
    let addr = addr
        .parse()
        .context("cannot parse the Manager server address")?;
    Ok(ManagerEndpoint {
        name: name.to_string(),
        addr,
    })
}

async fn connect_with_retry(builder: &ConnectionBuilder, manager: &ManagerEndpoint) -> Connection {
    let mut delay = MIN_RECONNECT_DELAY;
    loop {
        tracing::info!(
            "Connecting to Manager server {} at {}",
            manager.name,
            manager.addr
        );
        match builder.connect().await {
            Ok(conn) => {
                tracing::info!(
                    "Connected to Manager server {} at {}",
                    manager.name,
                    manager.addr
                );
                return conn;
            }
            Err(err) => {
                tracing::error!(
                    "Failed to connect to Manager server {} at {}: {err:#}. Retrying...",
                    manager.name,
                    manager.addr
                );
            }
        }
        delay = std::cmp::min(MAX_RECONNECT_DELAY, delay * 2);
        tokio::time::sleep(delay).await;
    }
}

struct RoxydRequestHandler;

impl RoxydRequestHandler {
    fn unimplemented_request<T>(&self, request: &str) -> std::result::Result<T, String> {
        tracing::info!(request_code = request, "Received review-protocol request");
        tracing::warn!(request_code = request, "Handler not implemented");
        unimplemented!("review-protocol handlers are not implemented");
    }
}

#[async_trait]
// NOTE: Not supported in roxyd yet. Adding new RequestCodes requires changes in
// review-protocol and Manager before handlers can be implemented here.
impl Handler for RoxydRequestHandler {
    // TODO: Implement host lifecycle handlers.
    async fn reboot(&mut self) -> std::result::Result<(), String> {
        self.unimplemented_request("Reboot")
    }

    async fn shutdown(&mut self) -> std::result::Result<(), String> {
        self.unimplemented_request("Shutdown")
    }

    // TODO: Implement resource reporting handlers.
    async fn resource_usage(&mut self) -> std::result::Result<(String, ResourceUsage), String> {
        self.unimplemented_request("ResourceUsage")
    }

    async fn process_list(&mut self) -> std::result::Result<Vec<Process>, String> {
        self.unimplemented_request("ProcessList")
    }

    // TODO: Not supported here (default behavior)
    // roxy capabilities that do not have review-protocol RequestCodes yet:
    // - hwinfo uptime/version
    // - hostname get/set
    // - syslog get/set/init/start
    // - ntp get/set/start/stop
    // - interface list/get/set/init/remove
    // - service_control (start/stop/restart/status)
    // - sshd get/start
    // - os/product version set
    // - graceful reboot/power off
}

pub async fn run_connection_loop(config: &RoxydConfig) -> Result<()> {
    let manager = parse_manager_server(&config.manager_server)?;
    let cert_pem = fs::read(&config.cert)
        .with_context(|| format!("failed to read certificate file: {}", config.cert.display()))?;
    let key_pem = fs::read(&config.key)
        .with_context(|| format!("failed to read private key file: {}", config.key.display()))?;
    let mut ca_certs_pem = Vec::new();
    for ca_cert in &config.ca_certs {
        let file = fs::read(ca_cert).with_context(|| {
            format!("failed to read CA certificate file: {}", ca_cert.display())
        })?;
        ca_certs_pem.push(file);
    }

    let mut conn_builder = ConnectionBuilder::new(
        &manager.name,
        manager.addr,
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        REQUIRED_REVIEW_VERSION,
        Status::Ready,
        &cert_pem,
        &key_pem,
    )
    .context("failed to create the review-protocol connection builder")?;
    conn_builder
        .root_certs(ca_certs_pem.iter())
        .context("failed to load CA certificates")?;

    let mut conn = connect_with_retry(&conn_builder, &manager).await;
    let mut handler = RoxydRequestHandler;
    loop {
        let (mut send, mut recv) = match conn.accept_bi().await {
            Ok((send, recv)) => (send, recv),
            Err(err) => {
                tracing::warn!(
                    "Connection closed: {err}. Reconnecting to Manager server {}",
                    manager.name
                );
                conn = connect_with_retry(&conn_builder, &manager).await;
                continue;
            }
        };
        if let Err(err) = request::handle(&mut handler, &mut send, &mut recv).await {
            tracing::warn!("Failed to handle review-protocol request: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    use review_protocol::types::Status;
    use rustls::pki_types::PrivatePkcs8KeyDer;
    use tokio::sync::oneshot;

    use super::REQUIRED_REVIEW_VERSION;

    #[tokio::test]
    async fn quic_handshake_completes() {
        const SERVER_NAME: &str = "localhost";

        let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("ca params");
        ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
        let ca_key = rcgen::KeyPair::generate().expect("ca key");
        let ca_cert = ca_params.self_signed(&ca_key).expect("ca cert");

        let server_key = rcgen::KeyPair::generate().expect("server key");
        let server_params =
            rcgen::CertificateParams::new([SERVER_NAME.to_string()]).expect("server params");
        let server_cert = server_params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .expect("server cert");

        let client_key = rcgen::KeyPair::generate().expect("client key");
        let client_params =
            rcgen::CertificateParams::new([SERVER_NAME.to_string()]).expect("client params");
        let client_cert = client_params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .expect("client cert");

        let cert_der = vec![server_cert.der().clone()];
        let key_der = PrivatePkcs8KeyDer::from(server_key.serialize_der());

        let server_config = quinn::ServerConfig::with_single_cert(cert_der.clone(), key_der.into())
            .expect("server config");
        let server_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 0);
        let server_endpoint =
            quinn::Endpoint::server(server_config, server_addr).expect("server endpoint");
        let server_addr = server_endpoint.local_addr().expect("server addr");

        let (ready_tx, ready_rx) = oneshot::channel::<()>();
        let server_task = tokio::spawn(async move {
            let connecting = server_endpoint.accept().await.expect("accept");
            let connection = connecting.await.expect("server connect");
            let res = review_protocol::server::handshake(
                &connection,
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
                REQUIRED_REVIEW_VERSION,
                REQUIRED_REVIEW_VERSION,
            )
            .await;
            if res.is_ok() {
                let _ = ready_rx.await;
            }
            res.map(|_| ())
        });

        let cert_pem = client_cert.pem();
        let key_pem = client_key.serialize_pem();
        let ca_pems = [ca_cert.pem().into_bytes()];

        let mut builder = review_protocol::client::ConnectionBuilder::new(
            SERVER_NAME,
            server_addr,
            env!("CARGO_PKG_NAME"),
            env!("CARGO_PKG_VERSION"),
            REQUIRED_REVIEW_VERSION,
            Status::Ready,
            cert_pem.as_bytes(),
            key_pem.as_bytes(),
        )
        .expect("builder");
        builder.root_certs(ca_pems.iter()).expect("root certs");

        let conn_res = builder.connect().await;
        if conn_res.is_ok() {
            let _ = ready_tx.send(());
        }
        let server_res = server_task.await;
        match (conn_res, server_res) {
            (Ok(_conn), Ok(Ok(()))) => {}
            (Err(err), Ok(server)) => {
                panic!("client connect: {err:?}, server: {server:?}");
            }
            (Ok(_conn), Ok(Err(err))) => panic!("server handshake failed: {err:?}"),
            (Ok(_conn), Err(err)) => panic!("server task failed: {err:?}"),
            (Err(err), Err(server_err)) => {
                panic!("client connect: {err:?}, server task failed: {server_err:?}");
            }
        }
    }
}
