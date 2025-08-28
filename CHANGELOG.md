# Changelog

This file documents recent notable changes to this project. The format of this
file is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.5.0] - 2025-08-28

### Added

- Add `graceful_reboot()` and `graceful_power_off()` functions for graceful
  system shutdown operations that allow processes to terminate cleanly

### Changed

- Improved disk usage calculation accuracy to match `df` command output
  - The file system that calculates disk usage is `/opt/clumit/var`, and if that
    file system cannot be found, `/` is used on Linux, and the largest file
    system on non-Linux
  - On Linux: Use `statvfs` syscall via nix crate instead of sysinfo for precise
    disk space calculations
  - Updated `ResourceUsage` struct: replaced `total_disk_space` and
    `used_disk_space` fields with `disk_used_bytes` and `disk_available_bytes`
  - Added `disk_usage_percentage()` method that calculates usage percentage using
    the same formula as `df`: `(used_space / (used_space + available_space)) * 100`
  - Non-Linux platforms continue to use sysinfo as fallback
- Migrate logging from `log` crate to `tracing` for improved async support and
  structured logging capabilities.

## [0.4.0] - 2025-07-04

### Changed

- Update roxy PATH from `/usr/local/aice/bin` to `/opt/clumit/bin`.
- Update disk mount PATH from `/data` to `/opt/clumit/var`.
- Update `log_debug` PATH from `/data/logs/apps` to `/opt/clumit/log`.
- Bump bincode crate to 2.0 and modified the related code.

## [0.3.0] - 2024-10-07

### Added

- Add `syslog, ssh, ntp` control function.

### Changed

- Limit the PATH of `roxy` program to `/usr/local/aice/bin`
- Apply rustfmt's option `group_imports=StdExternalCrate`.
  - Modify the code with the command `cargo fmt -- --config group_imports=StdExternalCrate`.
    This command must be applied automatically or manually before all future pull
    requests are submitted.
  - Add `--config group_imports=StdExternalCrate` to the CI process like:
    - `cargo fmt -- --check --config group_imports=StdExternalCrate`
- Bump systemctl crate to 0.4.0 and modify the related code.

## [0.2.1] - 2023-09-06

### Added

- Add `process_list` function to return a list of processes.

## [0.2.0] - 2023-03-22

### Added

- Add `service start|stop|status` command.

### Changed

- `uptime` returns `Duration` rather than `String`.

### Security

- Turned off the default features of chrono that might casue SEGFAULT. See
  [RUSTSEC-2020-0071](https://rustsec.org/advisories/RUSTSEC-2020-0071) for details.

## [0.1.0] - 2022-11-15

### Added

- Initial release.

[0.5.0]: https://github.com/aicers/roxy/compare/0.4.0...0.5.0
[0.4.0]: https://github.com/aicers/roxy/compare/0.3.0...0.4.0
[0.3.0]: https://github.com/aicers/roxy/compare/0.2.1...0.3.0
[0.2.1]: https://github.com/aicers/roxy/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/aicers/roxy/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/aicers/roxy/tree/0.1.0
