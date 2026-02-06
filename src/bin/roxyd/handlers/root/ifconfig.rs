#![allow(dead_code)]
#![allow(clippy::unused_async)]

use roxy::common::NicOutput;

pub async fn list_interfaces(_prefix: Option<String>) -> Result<Vec<String>, String> {
    unimplemented!("list_interfaces handler is not implemented")
}

pub async fn get_interfaces(
    _dev: Option<String>,
) -> Result<Option<Vec<(String, NicOutput)>>, String> {
    unimplemented!("get_interfaces handler is not implemented")
}

pub async fn set_interface(_dev: String, _nic: NicOutput) -> Result<String, String> {
    unimplemented!("set_interface handler is not implemented")
}

pub async fn init_interface(_dev: String) -> Result<String, String> {
    unimplemented!("init_interface handler is not implemented")
}

pub async fn remove_interface(_dev: String) -> Result<String, String> {
    unimplemented!("remove_interface handler is not implemented")
}
