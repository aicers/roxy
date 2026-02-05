#![allow(dead_code)]
#![allow(clippy::unused_async)]

pub async fn service_control(
    _subcmd: roxy::common::SubCommand,
    _service: String,
) -> Result<bool, String> {
    unimplemented!("service_control handler is not implemented")
}
