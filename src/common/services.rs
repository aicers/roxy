use std::{
    net::{IpAddr, SocketAddr, TcpStream},
    thread,
    time::{Duration, SystemTime},
};

use anyhow::Result;

/// Check the port is open (service is available).
/// * Be careful! The opened ports does not mean that service is available. Sometimes it takes more time.
/// * The service running in docker container should wait more time until service is ready.
///
/// # Errors
///
/// * invalid ipaddress or port number
pub fn waitfor_up(addr: &str, port: &str, timeout: u64) -> Result<bool> {
    let remote_sock = SocketAddr::new(addr.parse::<IpAddr>()?, port.parse::<u16>()?);
    let start = SystemTime::now();
    loop {
        match TcpStream::connect_timeout(&remote_sock, Duration::from_secs(1)) {
            Ok(_) => return Ok(true),
            Err(_) => {
                if SystemTime::now().duration_since(start)?.as_secs() < timeout {
                    thread::sleep(Duration::from_secs(1));
                } else {
                    return Ok(false);
                }
            }
        }
    }
}
