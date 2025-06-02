use std::path::Path;

use serde::{Deserialize, Serialize};

/// CPU, memory, and disk usage.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResourceUsage {
    /// The average CPU usage in percent.
    pub cpu_usage: f32,

    /// The RAM size in bytes.
    pub total_memory: u64,

    /// The amount of used RAM in bytes.
    pub used_memory: u64,

    /// The total disk space in bytes.
    pub total_disk_space: u64,

    /// The total disk space in bytes that is currently used.
    pub used_disk_space: u64,
}

/// Returns CPU, memory, and disk usage.
pub async fn resource_usage() -> ResourceUsage {
    use sysinfo::{Disks, RefreshKind, System};

    let mut system = System::new_with_specifics(RefreshKind::everything().without_processes());
    let (total_disk_space, used_disk_space) = {
        let disks = Disks::new_with_refreshed_list();
        if let Some(d) = disks
            .iter()
            .find(|&disk| disk.mount_point() == Path::new("/opt/clumit/var"))
        {
            (d.total_space(), d.total_space() - d.available_space())
        } else {
            // Find the disk with the largest space if `/opt/clumit/var` is not found
            if let Some(d) = disks.iter().max_by_key(|&disk| disk.total_space()) {
                (d.total_space(), d.total_space() - d.available_space())
            } else {
                (0, 0)
            }
        }
    };

    // Calculating CPU usage requires a time interval.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    system.refresh_cpu_usage();

    ResourceUsage {
        cpu_usage: system.global_cpu_usage(),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        total_disk_space,
        used_disk_space,
    }
}
