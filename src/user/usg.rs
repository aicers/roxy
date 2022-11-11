use serde::{Deserialize, Serialize};
use std::path::Path;

/// CPU, memory, and disk usage.
#[derive(Debug, Deserialize, Serialize)]
pub struct ResourceUsage {
    /// The average CPU usage in percent.
    pub cpu_usage: f32,

    /// The RAM size in KB.
    pub total_memory: u64,

    /// The amount of used RAM in KB.
    pub used_memory: u64,

    /// The total disk space in bytes.
    pub total_disk_space: u64,

    /// The total disk space in bytes that is currently used.
    pub used_disk_space: u64,
}

/// Returns CPU, memory, and disk usage.
pub async fn resource_usage() -> ResourceUsage {
    use sysinfo::{CpuExt, CpuRefreshKind, DiskExt, RefreshKind, System, SystemExt};

    let refresh = RefreshKind::new()
        .with_cpu(CpuRefreshKind::new().with_cpu_usage())
        .with_disks_list()
        .with_memory();
    let mut system = System::new_with_specifics(refresh);

    let (total_disk_space, used_disk_space) = {
        let disks = system.disks();
        if let Some(d) = disks
            .iter()
            .find(|&disk| disk.mount_point() == Path::new("/data"))
        {
            (d.total_space(), d.total_space() - d.available_space())
        } else {
            // Find the disk with the largest space if `/data` is not found
            if let Some(d) = disks.iter().max_by_key(|&disk| disk.total_space()) {
                (d.total_space(), d.total_space() - d.available_space())
            } else {
                (0, 0)
            }
        }
    };

    // Calculating CPU usage requires a time interval.
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    system.refresh_cpu();

    ResourceUsage {
        cpu_usage: system.global_cpu_info().cpu_usage(),
        total_memory: system.total_memory(),
        used_memory: system.used_memory(),
        total_disk_space,
        used_disk_space,
    }
}
