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

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    #[test]
    fn test_waitfor_up_returns_true_when_port_open() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .expect("localhost:0 should yield an ephemeral port for tests");
        let port = listener
            .local_addr()
            .expect("listener was just bound, so local_addr must be available")
            .port();

        let is_up = waitfor_up("127.0.0.1", &port.to_string(), 1)
            .expect("valid ip/port and stable system time should not error");

        assert!(is_up);
    }

    #[test]
    fn test_waitfor_up_returns_false_when_destination_port_zero() {
        // Destination port 0 cannot be an established listening service target.
        let is_up = waitfor_up("127.0.0.1", "0", 0)
            .expect("valid ip/port and stable system time should not error");

        assert!(!is_up);
    }

    #[test]
    fn test_waitfor_up_retries_until_timeout_for_destination_port_zero() {
        let start = std::time::Instant::now();
        let is_up = waitfor_up("127.0.0.1", "0", 1)
            .expect("valid ip/port and stable system time should not error");

        assert!(!is_up);
        assert!(start.elapsed() >= Duration::from_millis(900));
    }

    #[test]
    fn test_waitfor_up_invalid_ip_fails() {
        assert!(waitfor_up("invalid-ip", "80", 1).is_err());
    }

    #[test]
    fn test_waitfor_up_invalid_port_fails() {
        assert!(waitfor_up("127.0.0.1", "not-a-port", 1).is_err());
    }
}
