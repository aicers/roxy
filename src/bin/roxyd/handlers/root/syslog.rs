#![allow(dead_code)]
#![allow(clippy::unused_async)]

pub async fn get_syslog_servers() -> Result<Option<Vec<(String, String, String)>>, String> {
    unimplemented!("syslog get_syslog_servers handler is not implemented")
}

pub async fn set_syslog_servers(_servers: Vec<String>) -> Result<String, String> {
    unimplemented!("syslog set_syslog_servers handler is not implemented")
}

pub async fn init_syslog_servers() -> Result<String, String> {
    unimplemented!("syslog init_syslog_servers handler is not implemented")
}

pub async fn start_syslog_servers() -> Result<bool, String> {
    unimplemented!("syslog start_syslog_servers handler is not implemented")
}
