use std::path::Path;

use serde::{Deserialize, Serialize};

/// Multiplies two u64 values with saturation at `u64::MAX`.
///
/// Uses u128 intermediate to detect overflow and clamps result to `u64::MAX`
/// instead of panicking (debug) or wrapping (release). See issue #571.
#[cfg(target_os = "linux")]
fn saturating_mul_u64(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    if product > u128::from(u64::MAX) {
        u64::MAX
    } else {
        product as u64
    }
}

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
    ///
    /// Uses saturating arithmetic to prevent overflow/underflow across debug and
    /// release builds. See issue #571.
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn disk_usage_percentage(&self) -> f32 {
        // Use saturating_add to prevent overflow in debug builds and wrapping in
        // release builds.
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
/// Uses saturating arithmetic to prevent overflow/underflow across debug and release builds.
/// See issue #571.
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
                // Space used by non-root users (matches df calculation).
                // Use saturating_sub to prevent underflow if blocks_free > blocks.
                // Use wide multiplication (u128) then clamp to u64::MAX to prevent
                // overflow.
                let used_blocks = stat.blocks().saturating_sub(stat.blocks_free());
                let used_space = saturating_mul_u64(used_blocks, block_size);
                // Space available to non-root users
                let available_space = saturating_mul_u64(stat.blocks_available(), block_size);
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
            // Use saturating_sub to prevent underflow if available_space > total_space.
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
                        // Use saturating_sub to prevent underflow. See issue #571.
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

    // ==========================================================================
    // Overflow/underflow boundary tests (issue #571)
    //
    // These tests verify that arithmetic operations are safe and deterministic
    // across both debug and release builds. They do not rely on panic assertions.
    // ==========================================================================

    #[test]
    fn test_disk_usage_percentage_overflow_both_max() {
        // Both used and available at u64::MAX would overflow with raw addition.
        // With saturating_add, total becomes u64::MAX and percentage should be ~50%.
        let usage = resource_usage_with_disk(u64::MAX, u64::MAX);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        // When both values are u64::MAX, total saturates to u64::MAX.
        // percentage = u64::MAX / u64::MAX * 100 = 100%
        // This is expected because saturating_add(MAX, MAX) = MAX, and used = MAX.
        assert_percentage_close(percentage, 100.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_used_max_available_one() {
        // used = u64::MAX, available = 1 would overflow with raw addition.
        // With saturating_add, total = u64::MAX and percentage should be ~100%.
        let usage = resource_usage_with_disk(u64::MAX, 1);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 100.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_used_one_available_max() {
        // used = 1, available = u64::MAX would overflow with raw addition.
        // With saturating_add, total = u64::MAX and percentage should be ~0%.
        let usage = resource_usage_with_disk(1, u64::MAX);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        assert_percentage_close(percentage, 0.0, 0.001);
    }

    #[test]
    fn test_disk_usage_percentage_overflow_half_max_plus_one() {
        // Values that would overflow: (MAX/2 + 1) + (MAX/2 + 1) > MAX
        let half_plus_one = u64::MAX / 2 + 1;
        let usage = resource_usage_with_disk(half_plus_one, half_plus_one);
        let percentage = usage.disk_usage_percentage();
        assert!(percentage.is_finite());
        // With saturation, total = u64::MAX, so percentage = half_plus_one / MAX * 100 ≈ 50%
        assert_percentage_close(percentage, 50.0, 1.0);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_saturating_mul_u64_no_overflow() {
        // Normal multiplication should work as expected
        assert_eq!(saturating_mul_u64(100, 200), 20_000);
        assert_eq!(saturating_mul_u64(0, u64::MAX), 0);
        assert_eq!(saturating_mul_u64(u64::MAX, 0), 0);
        assert_eq!(saturating_mul_u64(1, u64::MAX), u64::MAX);
        assert_eq!(saturating_mul_u64(u64::MAX, 1), u64::MAX);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_saturating_mul_u64_overflow_clamps_to_max() {
        // Multiplication that would overflow should clamp to u64::MAX
        assert_eq!(saturating_mul_u64(u64::MAX, 2), u64::MAX);
        assert_eq!(saturating_mul_u64(2, u64::MAX), u64::MAX);
        assert_eq!(saturating_mul_u64(u64::MAX, u64::MAX), u64::MAX);
        // Large values that would overflow
        let large = u64::MAX / 2 + 1;
        assert_eq!(saturating_mul_u64(large, 3), u64::MAX);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_saturating_mul_u64_boundary() {
        // Test boundary case: largest multiplication that doesn't overflow
        // sqrt(u64::MAX) ≈ 4_294_967_296 (2^32)
        let sqrt_max = 1u64 << 32;
        // (2^32 - 1) * (2^32 - 1) should not overflow
        let below_sqrt = sqrt_max - 1;
        let result = saturating_mul_u64(below_sqrt, below_sqrt);
        assert!(result < u64::MAX);
        assert_eq!(result, below_sqrt * below_sqrt);

        // (2^32) * (2^32) = 2^64 which overflows u64
        assert_eq!(saturating_mul_u64(sqrt_max, sqrt_max), u64::MAX);
    }
}
