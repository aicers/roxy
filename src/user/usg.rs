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

    /// The disk space in bytes that is currently used.
    pub disk_used_bytes: u64,

    /// The disk space in bytes that is available to non-root users.
    pub disk_available_bytes: u64,
}

impl ResourceUsage {
    /// Calculates disk usage percentage using the same formula as `df`.
    ///
    /// Formula: (`used_space` / (`used_space` + `available_space`)) * 100
    ///
    /// Returns 0.0 if no disk space information is available.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn disk_usage_percentage(&self) -> f32 {
        let total = self.disk_used_bytes + self.disk_available_bytes;
        if total == 0 {
            0.0
        } else {
            (self.disk_used_bytes as f32 / total as f32) * 100.0
        }
    }
}

/// Returns accurate disk space information using statvfs on Linux, fallback to sysinfo otherwise.
///
/// # Errors
///
/// Returns error if disk space calculation fails on the target platform.
fn get_disk_usage(mount_point: &Path) -> Result<(u64, u64), Box<dyn std::error::Error>> {
    #[cfg(target_os = "linux")]
    {
        use nix::sys::statvfs;

        match statvfs::statvfs(mount_point) {
            Ok(stat) => {
                let block_size = stat.fragment_size();
                // Space used by non-root users (matches df calculation)
                let used_space = (stat.blocks() - stat.blocks_free()) * block_size;
                // Space available to non-root users
                let available_space = stat.blocks_available() * block_size;
                Ok((used_space, available_space))
            }
            Err(e) => Err(Box::new(e)),
        }
    }

    #[cfg(not(target_os = "linux"))]
    {
        // Fallback to sysinfo for non-Linux platforms
        use sysinfo::Disks;

        let disks = Disks::new_with_refreshed_list();
        if let Some(d) = disks.iter().find(|&disk| disk.mount_point() == mount_point) {
            let used_space = d.total_space() - d.available_space();
            let available_space = d.available_space();
            Ok((used_space, available_space))
        } else {
            Err("Mount point not found".into())
        }
    }
}

/// Returns CPU, memory, and disk usage.
pub async fn resource_usage() -> ResourceUsage {
    use sysinfo::{RefreshKind, System};

    let mut system = System::new_with_specifics(RefreshKind::everything().without_processes());

    let (disk_used_bytes, disk_available_bytes) = {
        let target_mount = Path::new("/opt/clumit/var");

        match get_disk_usage(target_mount) {
            Ok((used, available)) => (used, available),
            Err(_) => {
                // Fallback: Find the disk with the largest space if `/opt/clumit/var` is not found
                #[cfg(not(target_os = "linux"))]
                {
                    use sysinfo::Disks;

                    let disks = Disks::new_with_refreshed_list();
                    if let Some(d) = disks.iter().max_by_key(|&disk| disk.total_space()) {
                        let used = d.total_space() - d.available_space();
                        let available = d.available_space();
                        (used, available)
                    } else {
                        (0, 0)
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    // On Linux, try root filesystem as fallback
                    match get_disk_usage(Path::new("/")) {
                        Ok((used, available)) => (used, available),
                        Err(_) => (0, 0),
                    }
                }
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
        disk_used_bytes,
        disk_available_bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_disk_usage_percentage() {
        let usage = ResourceUsage {
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            disk_used_bytes: 80_000_000_000,      // 80GB used
            disk_available_bytes: 20_000_000_000, // 20GB available
        };

        // Total: 100GB, Used: 80GB -> 80%
        assert_eq!(usage.disk_usage_percentage(), 80.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_disk_usage_percentage_zero_disk() {
        let usage = ResourceUsage {
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            disk_used_bytes: 0,
            disk_available_bytes: 0,
        };

        assert_eq!(usage.disk_usage_percentage(), 0.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_disk_usage_percentage_no_used_space() {
        let usage = ResourceUsage {
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            disk_used_bytes: 0,
            disk_available_bytes: 100_000_000_000, // 100GB available
        };

        assert_eq!(usage.disk_usage_percentage(), 0.0);
    }

    #[test]
    #[allow(clippy::float_cmp)]
    fn test_disk_usage_percentage_realistic_values() {
        let usage = ResourceUsage {
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            disk_used_bytes: 45_000_000_000,      // 45GB used
            disk_available_bytes: 55_000_000_000, // 55GB available
        };

        // Total: 100GB, Used: 45GB -> 45%
        assert_eq!(usage.disk_usage_percentage(), 45.0);
    }
}
