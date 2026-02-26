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
        let total = self
            .disk_used_bytes
            .saturating_add(self.disk_available_bytes);
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
                let used_blocks = stat.blocks().saturating_sub(stat.blocks_free());
                let used_space = used_blocks.saturating_mul(block_size);
                let available_space = stat.blocks_available().saturating_mul(block_size);
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
            let used_space = d.total_space().saturating_sub(d.available_space());
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
                        let used = d.total_space().saturating_sub(d.available_space());
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

    fn resource_usage_with_disk(disk_used_bytes: u64, disk_available_bytes: u64) -> ResourceUsage {
        ResourceUsage {
            cpu_usage: 0.0,
            total_memory: 0,
            used_memory: 0,
            disk_used_bytes,
            disk_available_bytes,
        }
    }

    fn assert_percentage_close(actual: f32, expected: f32, epsilon: f32) {
        assert!((actual - expected).abs() <= epsilon);
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn test_get_disk_usage_root_mount() {
        let (used, available) =
            get_disk_usage(Path::new("/")).expect("failed to get disk usage for root mount");
        assert!(used.saturating_add(available) > 0);
    }

    #[test]
    fn test_get_disk_usage_missing_mount_point() {
        let missing_mount = Path::new("/__roxy_test_missing_mount_point__");
        assert!(get_disk_usage(missing_mount).is_err());
    }

    #[test]
    fn test_disk_usage_percentage() {
        let usage = resource_usage_with_disk(80_000_000_000, 20_000_000_000);
        assert_percentage_close(usage.disk_usage_percentage(), 80.0, 0.000_1);
    }

    #[test]
    fn test_disk_usage_percentage_zero_disk() {
        let usage = resource_usage_with_disk(0, 0);
        assert_percentage_close(usage.disk_usage_percentage(), 0.0, 0.0);
    }

    #[test]
    fn test_disk_usage_percentage_no_used_space() {
        let usage = resource_usage_with_disk(0, 100_000_000_000);
        assert_percentage_close(usage.disk_usage_percentage(), 0.0, 0.0);
    }

    #[test]
    fn test_disk_usage_percentage_full_disk() {
        let usage = resource_usage_with_disk(100_000_000_000, 0);
        assert_percentage_close(usage.disk_usage_percentage(), 100.0, 0.000_1);
    }

    #[test]
    fn test_disk_usage_percentage_large_symmetric_values() {
        let usage = resource_usage_with_disk(u64::MAX / 2, u64::MAX / 2);
        assert_percentage_close(usage.disk_usage_percentage(), 50.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_fractional() {
        let usage = resource_usage_with_disk(1, 3);
        assert_percentage_close(usage.disk_usage_percentage(), 25.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_non_round_fraction() {
        let usage = resource_usage_with_disk(1, 2);
        assert_percentage_close(usage.disk_usage_percentage(), 100.0 / 3.0, 0.01);
    }

    #[test]
    fn test_disk_usage_percentage_near_max_total() {
        let usage = resource_usage_with_disk(u64::MAX - 1, 1);
        assert_percentage_close(usage.disk_usage_percentage(), 100.0, 0.001);
    }

    #[tokio::test]
    async fn test_resource_usage_smoke() {
        let usage = resource_usage().await;
        assert!(usage.cpu_usage.is_finite());
        assert!(usage.cpu_usage >= 0.0);
        assert!(usage.cpu_usage <= 100.0);
        assert!(usage.used_memory <= usage.total_memory);
        assert!(usage.disk_usage_percentage().is_finite());
        assert!((0.0..=100.0).contains(&usage.disk_usage_percentage()));
    }

    #[test]
    fn test_disk_usage_percentage_overflow_both_max() {
        // saturating_add(MAX, MAX) = MAX, so percentage = MAX / MAX * 100 = 100%.
        let usage = resource_usage_with_disk(u64::MAX, u64::MAX);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 100.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_used_max_available_one() {
        let usage = resource_usage_with_disk(u64::MAX, 1);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 100.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_used_one_available_max() {
        let usage = resource_usage_with_disk(1, u64::MAX);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 0.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_half_max_plus_one() {
        let half_plus_one = u64::MAX / 2 + 1;
        let usage = resource_usage_with_disk(half_plus_one, half_plus_one);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 50.0, 1.0);
    }

    #[test]
    fn test_saturating_sub_underflow_clamps_to_zero() {
        // Simulates the used-space calculation when available > total,
        // which would underflow with raw subtraction.
        let total: u64 = 100;
        let available: u64 = 200;
        let used = total.saturating_sub(available);
        assert_eq!(used, 0);

        // Extreme case: 0 - MAX
        assert_eq!(0u64.saturating_sub(u64::MAX), 0);

        // Normal case: total > available
        assert_eq!(500u64.saturating_sub(300), 200);
    }
}
