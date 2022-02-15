use roxy::task::{ExecResult, Task, ERR_INVALID_COMMAND};
use std::{
    io::{stdin, stdout},
    process,
};

fn main() {
    let task: Task = match serde_json::from_reader(stdin()) {
        Ok(task) => task,
        Err(err) => {
            log::error!("Command Error: {}", err);
            if let Err(err) =
                serde_json::to_writer_pretty(stdout(), &ExecResult::Err(ERR_INVALID_COMMAND))
            {
                log::error!("Serialize Error: {}", err);
            }
            process::exit(1);
        }
    };

    let ret = task.execute();
    if let Err(err) = serde_json::to_writer_pretty(stdout(), &ret) {
        log::error!("Stdout Error: {}", err);
        process::exit(1);
    }
}
