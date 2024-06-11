# Changelog

This file documents recent notable changes to this project. The format of this
file is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/), and
this project adheres to [Semantic
Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
  [RUSTSEC-2020-0071](https://rustsec.org/advisories/RUSTSEC-2020-0071)
  for details.

## [0.1.0] - 2022-11-15

### Added

- Initial release.

[0.2.1]: https://github.com/aicers/roxy/compare/0.2.0...0.2.1
[0.2.0]: https://github.com/aicers/roxy/compare/0.1.0...0.2.0
[0.1.0]: https://github.com/aicers/roxy/tree/0.1.0
