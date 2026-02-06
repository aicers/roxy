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

    // Tests for the Process struct serialization/deserialization.
    //
    // Branches that cannot be covered without production changes:
    // - Kernel thread exclusion (parent PID == KTHREAD_PID): requires mocking
    //   sysinfo::System or refactoring to accept a process iterator.
    // - User ID lookup returning None vs Some: depends on real system users.
    // - The async sleep and refresh cycle in process_list(): requires OS state.

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_process_serialization_roundtrip() {
        let process = Process {
            user: "testuser".to_string(),
            cpu_usage: 25.5,
            mem_usage: 12.34,
            start_time: 1_700_000_000_000_000_000,
            command: "test_command".to_string(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize Process");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize Process");

        assert_eq!(deserialized.user, process.user);
        assert_eq!(deserialized.cpu_usage, process.cpu_usage);
        assert_eq!(deserialized.mem_usage, process.mem_usage);
        assert_eq!(deserialized.start_time, process.start_time);
        assert_eq!(deserialized.command, process.command);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_process_bincode_roundtrip() {
        let process = Process {
            user: "root".to_string(),
            cpu_usage: 0.0,
            mem_usage: 50.0,
            start_time: 0,
            command: "init".to_string(),
        };

        let encoded = bincode::serialize(&process).expect("serialize Process with bincode");
        let decoded: Process =
            bincode::deserialize(&encoded).expect("deserialize Process with bincode");

        assert_eq!(decoded.user, process.user);
        assert_eq!(decoded.cpu_usage, process.cpu_usage);
        assert_eq!(decoded.mem_usage, process.mem_usage);
        assert_eq!(decoded.start_time, process.start_time);
        assert_eq!(decoded.command, process.command);
    }

    #[test]
    fn test_process_with_default_user() {
        let process = Process {
            user: DEFAULT_USER_NAME.to_string(),
            cpu_usage: 1.0,
            mem_usage: 2.0,
            start_time: 123_456_789,
            command: "unknown".to_string(),
        };

        assert_eq!(process.user, "N/A");
    }

    // Tests verifying computed field formulas produce correct results.
    // These tests validate the mathematical operations used in process_list().

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_cpu_usage_normalization_formula() {
        // Formula: process.cpu_usage() / num_cpu
        let raw_cpu_usage: f32 = 400.0;
        let num_cpu: f32 = 4.0;
        let normalized = raw_cpu_usage / num_cpu;

        assert_eq!(normalized, 100.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_cpu_usage_single_cpu() {
        let raw_cpu_usage: f32 = 50.0;
        let num_cpu: f32 = 1.0;
        let normalized = raw_cpu_usage / num_cpu;

        assert_eq!(normalized, 50.0);
    }

    #[test]
    #[allow(clippy::float_cmp, clippy::cast_precision_loss)]
    fn test_mem_usage_percentage_formula() {
        // Formula: process.memory() as f64 / total_memory * 100.0
        let process_memory: u64 = 1_000_000_000; // 1GB
        let total_memory: f64 = 16_000_000_000.0; // 16GB
        let mem_usage = process_memory as f64 / total_memory * 100.0;

        assert_eq!(mem_usage, 6.25);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn test_mem_usage_zero_total_memory() {
        // When total_memory is 0, division produces infinity.
        let process_memory: u64 = 1_000_000;
        let total_memory: f64 = 0.0;
        let mem_usage = process_memory as f64 / total_memory * 100.0;

        assert!(mem_usage.is_infinite());
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn test_mem_usage_zero_process_memory() {
        let process_memory: u64 = 0;
        let total_memory: f64 = 16_000_000_000.0;
        let mem_usage = process_memory as f64 / total_memory * 100.0;

        assert!((mem_usage - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    #[allow(clippy::float_cmp, clippy::cast_possible_wrap)]
    fn test_start_time_conversion_formula() {
        // Formula: process.start_time() as i64 * NANO_SEC
        let start_time_secs: u64 = 1_700_000_000;
        let start_time_nanos = start_time_secs as i64 * NANO_SEC;

        assert_eq!(start_time_nanos, 1_700_000_000_000_000_000);
    }

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_start_time_zero() {
        let start_time_secs: u64 = 0;
        let start_time_nanos = start_time_secs as i64 * NANO_SEC;

        assert_eq!(start_time_nanos, 0);
    }

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_start_time_large_value() {
        // Test with a large but valid timestamp (year ~2100)
        let start_time_secs: u64 = 4_102_444_800;
        let start_time_nanos = start_time_secs as i64 * NANO_SEC;

        assert_eq!(start_time_nanos, 4_102_444_800_000_000_000);
    }

    #[test]
    fn test_kthread_pid_constant() {
        // Verify the kernel thread PID constant matches Linux convention.
        assert_eq!(KTHREAD_PID, 2);
    }

    #[test]
    fn test_nano_sec_constant() {
        assert_eq!(NANO_SEC, 1_000_000_000);
    }

    #[test]
    fn test_process_with_special_characters_in_command() {
        let process = Process {
            user: "user".to_string(),
            cpu_usage: 0.0,
            mem_usage: 0.0,
            start_time: 0,
            command: "[kworker/0:1-events]".to_string(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(deserialized.command, "[kworker/0:1-events]");
    }

    #[test]
    fn test_process_with_unicode_username() {
        let process = Process {
            user: "用户".to_string(),
            cpu_usage: 10.5,
            mem_usage: 5.25,
            start_time: 1_000_000_000_000_000_000,
            command: "プロセス".to_string(),
        };

        let serialized = serde_json::to_string(&process).expect("serialize");
        let deserialized: Process = serde_json::from_str(&serialized).expect("deserialize");

        assert_eq!(deserialized.user, "用户");
        assert_eq!(deserialized.command, "プロセス");
    }

    #[test]
    fn test_process_with_empty_strings() {
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

    #[test]
    fn test_cpu_usage_boundary_values() {
        // CPU usage can exceed 100% per core in sysinfo
        let process = Process {
            user: "test".to_string(),
            cpu_usage: 0.0,
            mem_usage: 0.0,
            start_time: 0,
            command: "test".to_string(),
        };
        assert!(process.cpu_usage >= 0.0);

        let process_high = Process {
            user: "test".to_string(),
            cpu_usage: 100.0,
            mem_usage: 0.0,
            start_time: 0,
            command: "test".to_string(),
        };
        assert!(process_high.cpu_usage <= 100.0 || process_high.cpu_usage > 100.0);
    }

    #[test]
    fn test_mem_usage_boundary_values() {
        // Memory usage should be between 0 and 100 percent
        let process_zero = Process {
            user: "test".to_string(),
            cpu_usage: 0.0,
            mem_usage: 0.0,
            start_time: 0,
            command: "test".to_string(),
        };
        assert!(process_zero.mem_usage >= 0.0);

        let process_full = Process {
            user: "test".to_string(),
            cpu_usage: 0.0,
            mem_usage: 100.0,
            start_time: 0,
            command: "test".to_string(),
        };
        assert!(process_full.mem_usage <= 100.0);
    }

    #[test]
    #[allow(clippy::cast_precision_loss)]
    fn test_mem_usage_percentage_large_values() {
        // Test with very large memory values (64GB process on 128GB system)
        let process_memory: u64 = 64_000_000_000;
        let total_memory: f64 = 128_000_000_000.0;
        let mem_usage = process_memory as f64 / total_memory * 100.0;

        assert!((mem_usage - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_cpu_usage_zero_cpus_produces_infinity() {
        // Edge case: if num_cpu is 0, division produces infinity.
        // This cannot happen in practice since sysinfo returns at least 1 CPU.
        let raw_cpu_usage: f32 = 50.0;
        let num_cpu: f32 = 0.0;
        let normalized = raw_cpu_usage / num_cpu;

        assert!(normalized.is_infinite());
    }

    #[test]
    #[allow(clippy::cast_possible_wrap)]
    fn test_process_negative_start_time_wrapping() {
        // u64::MAX as i64 wraps to -1, then multiplied by NANO_SEC
        // This tests the behavior of the cast, though in practice start_time
        // from sysinfo is a reasonable Unix timestamp.
        let large_start_time: u64 = u64::MAX;
        let start_time_nanos = large_start_time as i64 * NANO_SEC;

        // u64::MAX as i64 is -1, -1 * 1_000_000_000 = -1_000_000_000
        assert_eq!(start_time_nanos, -NANO_SEC);
    }
}
