#![allow(dead_code)]
#![allow(clippy::unused_async)]

pub async fn get_hostname() -> Result<String, String> {
    unimplemented!("get_hostname handler is not implemented")
}

pub async fn set_hostname(_name: &str) -> Result<(), String> {
    unimplemented!("set_hostname handler is not implemented")
}

pub async fn reboot() -> Result<(), String> {
    unimplemented!("reboot handler is not implemented")
}

pub async fn shutdown() -> Result<(), String> {
    unimplemented!("shutdown handler is not implemented")
}

pub async fn power_off() -> Result<(), String> {
    unimplemented!("power_off handler is not implemented")
}

pub async fn graceful_reboot() -> Result<(), String> {
    unimplemented!("graceful_reboot handler is not implemented")
}

pub async fn graceful_power_off() -> Result<(), String> {
    unimplemented!("graceful_power_off handler is not implemented")
}
