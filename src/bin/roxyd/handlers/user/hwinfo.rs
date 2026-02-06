#![allow(dead_code)]
#![allow(clippy::unused_async)]

use std::time::Duration;

pub async fn uptime() -> Result<Duration, String> {
    unimplemented!("uptime handler is not implemented")
}

pub async fn version() -> Result<(String, String), String> {
    unimplemented!("version handler is not implemented")
}
