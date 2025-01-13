use serde::{Deserialize, Serialize};
use sysinfo::{System, Users, MINIMUM_CPU_UPDATE_INTERVAL};

const KTHREAD_PID: u32 = 2;
const DEFAULT_USER_NAME: &str = "N/A";
const NANO_SEC: i64 = 1_000_000_000;

#[derive(Debug, Deserialize, Serialize)]
pub struct Process {
    pub user: String,
    pub cpu_usage: f32,
    pub mem_usage: f64,
    pub start_time: i64,
    pub command: String,
}

/// Returns processes's username, cpu usage, memory usage, start time, and command except kernel thread.
#[allow(
    clippy::module_name_repetitions,
    // start_time u64 to i64
    clippy::cast_possible_wrap,
    // memory u64 to f64
    clippy::cast_precision_loss
)]
#[must_use]
pub async fn process_list() -> Vec<Process> {
    let mut system = System::new_all();
    let mut processes = Vec::new();
    let users = Users::new_with_refreshed_list();

    // Calculating CPU usage requires a time interval.
    tokio::time::sleep(MINIMUM_CPU_UPDATE_INTERVAL).await;
    system.refresh_all();

    let total_memory = system.total_memory() as f64;
    let num_cpu = system.cpus().len() as f32;

    for process in system.processes().values() {
        if process
            .parent()
            .is_some_and(|ppid| ppid.as_u32() == KTHREAD_PID)
        {
            continue;
        }
        let user = process
            .user_id()
            .and_then(|uid| users.get_user_by_id(uid))
            .map_or(DEFAULT_USER_NAME, |u| u.name())
            .to_string();
        let cpu_usage = process.cpu_usage() / num_cpu;
        let mem_usage = process.memory() as f64 / total_memory * 100.0;
        let start_time = process.start_time() as i64 * NANO_SEC;
        let command = process.name().to_string_lossy().to_string();

        processes.push(Process {
            user,
            cpu_usage,
            mem_usage,
            start_time,
            command,
        });
    }

    processes
}
