use serde::{Deserialize, Serialize};
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, System, Users};

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_roundtrip() {
        let process = Process {
            user: "testuser".to_string(),
            cpu_usage: 25.5,
            mem_usage: 12.34,
            start_time: 1_700_000_000_000_000_000,
            command: "test_command".to_string(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(deserialized.user, process.user);
        assert!((deserialized.cpu_usage - process.cpu_usage).abs() < f32::EPSILON);
        assert!((deserialized.mem_usage - process.mem_usage).abs() < f64::EPSILON);
        assert_eq!(deserialized.start_time, process.start_time);
        assert_eq!(deserialized.command, process.command);
    }

    #[test]
    fn test_bincode_roundtrip() {
        let process = Process {
            user: "root".to_string(),
            cpu_usage: 0.0,
            mem_usage: 50.0,
            start_time: 0,
            command: "init".to_string(),
        };

        let encoded = bincode::serialize(&process).expect("serialize");
        let decoded: Process = bincode::deserialize(&encoded).expect("deserialize");

        assert_eq!(decoded.user, process.user);
        assert!((decoded.cpu_usage - process.cpu_usage).abs() < f32::EPSILON);
        assert!((decoded.mem_usage - process.mem_usage).abs() < f64::EPSILON);
        assert_eq!(decoded.start_time, process.start_time);
        assert_eq!(decoded.command, process.command);
    }

    #[test]
    fn test_json_roundtrip_with_special_characters() {
        let process = Process {
            user: "test-user".to_string(),
            cpu_usage: 10.5,
            mem_usage: 5.25,
            start_time: 1_000_000_000_000_000_000,
            command: "[kworker/0:1-events]".to_string(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(deserialized.user, "test-user");
        assert_eq!(deserialized.command, "[kworker/0:1-events]");
    }

    #[test]
    fn test_json_roundtrip_with_empty_strings() {
        let process = Process {
            user: String::new(),
            cpu_usage: 0.0,
            mem_usage: 0.0,
            start_time: 0,
            command: String::new(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize");

        assert!(deserialized.user.is_empty());
        assert!(deserialized.command.is_empty());
    }

    #[tokio::test]
    async fn test_process_list_fields_are_populated() {
        let processes = process_list().await;
        assert!(!processes.is_empty());

        for process in &processes {
            assert!(!process.command.is_empty());
            assert!(process.cpu_usage >= 0.0);
            assert!(process.mem_usage >= 0.0);
            assert!(process.start_time >= 0);
        }
    }

    #[tokio::test]
    async fn test_process_list_mem_usage_is_percentage_like() {
        let processes = process_list().await;
        assert!(!processes.is_empty());

        for process in &processes {
            assert!(process.mem_usage <= 100.0);
        }
    }
}
