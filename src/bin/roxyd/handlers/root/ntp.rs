#![allow(dead_code)]
#![allow(clippy::unused_async)]

pub async fn get_ntp() -> Result<Option<Vec<String>>, String> {
    unimplemented!("ntp get_ntp handler is not implemented")
}

pub async fn set_ntp(_servers: Vec<String>) -> Result<bool, String> {
    unimplemented!("ntp set_ntp handler is not implemented")
}

pub async fn start_ntp() -> Result<bool, String> {
    unimplemented!("ntp start_ntp handler is not implemented")
}

pub async fn stop_ntp() -> Result<bool, String> {
    unimplemented!("ntp stop_ntp handler is not implemented")
}
