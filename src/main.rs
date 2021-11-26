mod task;

use crate::task::Task;
use std::{
    io::{stdin, stdout},
    process,
};

type TaskResult = std::result::Result<(), &'static str>;

fn main() {
    let task: Task = match serde_json::from_reader(stdin()) {
        Ok(task) => task,
        Err(err) => {
            if let Err(err) =
                serde_json::to_writer_pretty(stdout(), &TaskResult::Err("invalid task"))
            {
                eprintln!("{}", err);
                process::exit(1);
            }
            eprintln!("{}", err);
            process::exit(1);
        }
    };
    task.execute();
    if let Err(err) = serde_json::to_writer_pretty(stdout(), &TaskResult::Ok(())) {
        eprintln!("{}", err);
        process::exit(1);
    }
}
