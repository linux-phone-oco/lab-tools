# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.6.1...lpoco-host-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.6.0...lpoco-host-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.5.0...lpoco-host-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.4.1...lpoco-host-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.4.0...lpoco-host-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.8...lpoco-host-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.7...lpoco-host-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.6...lpoco-host-v0.3.7) - 2026-11-18

### Other

- release v0.3.7 ([#118](https://github.com/linux-phone-oco/lab-tools/pull/118))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.5...lpoco-host-v0.3.6) - 2026-11-18

### Other

- release v0.3.5 ([#113](https://github.com/linux-phone-oco/lab-tools/pull/113))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.4...lpoco-host-v0.3.5) - 2026-11-18

### Other

- release v0.3.5 ([#111](https://github.com/linux-phone-oco/lab-tools/pull/111))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.3.3...lpoco-host-v0.3.4) - 2026-11-18

### Added

- lpoco host reads each device from origin/main and names the commit and its lag ([#108](https://github.com/linux-phone-oco/lab-tools/pull/108))

## [0.3.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.2.2...lpoco-host-v0.3.0) - 2026-11-18

### Added

- *(host)* script tools judged by a rev stamp, devices takes a list ([#95](https://github.com/linux-phone-oco/lab-tools/pull/95))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.2.1...lpoco-host-v0.2.2) - 2026-11-17

### Added

- --help and -h print the usage in lpoco, lpoco-host, lpoco-bringup and lpoco-health ([#93](https://github.com/linux-phone-oco/lab-tools/pull/93))

## [0.2.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.2.0...lpoco-host-v0.2.1) - 2026-11-17

### Added

- *(host)* lpoco host, the lab host prepared from lab.toml and each device's [host] ([#91](https://github.com/linux-phone-oco/lab-tools/pull/91))
