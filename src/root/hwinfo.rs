use std::{
    fs::{self, OpenOptions},
    io::Write as IoWrite,
};

use anyhow::{anyhow, Result};

use super::SubCommand;

// TODO: should change this path to /usr/local/aice/conf/version?
const DEFAULT_VERSION_PATH: &str = "/etc/version";

pub(crate) fn set_version(kind: SubCommand, arg: &str) -> Result<()> {
    let contents = fs::read_to_string(DEFAULT_VERSION_PATH)?;
    let lines = contents.lines();
    let mut new_contents = String::new();
    for line in lines {
        match kind {
            SubCommand::SetOsVersion => {
                if line.to_lowercase().starts_with("os:") {
                    continue;
                }
            }
            SubCommand::SetProductVersion => {
                if line.to_lowercase().starts_with("product:") {
                    continue;
                }
            }
            _ => return Err(anyhow!("invalid command")),
        }

        new_contents.push_str(line);
        new_contents.push('\n');
    }

    let new_version = match kind {
        SubCommand::SetOsVersion => format!("OS: {arg}"),
        SubCommand::SetProductVersion => format!("Product: {arg}"),
        _ => return Err(anyhow!("invalid command")),
    };

    new_contents.push_str(&new_version);
    new_contents.push('\n');

    let mut file = OpenOptions::new()
        .write(true)
        .truncate(true)
        .open(DEFAULT_VERSION_PATH)?;

    file.write_all(new_contents.as_bytes())?;
    Ok(())
}
