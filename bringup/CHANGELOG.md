# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.7.1...lpoco-bringup-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.7.0...lpoco-bringup-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.6.1...lpoco-bringup-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.6.0...lpoco-bringup-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.5.0...lpoco-bringup-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.4.1...lpoco-bringup-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.4.0...lpoco-bringup-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.8...lpoco-bringup-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.7...lpoco-bringup-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.6...lpoco-bringup-v0.3.7) - 2026-11-18

### Other

- release v0.3.7 ([#118](https://github.com/linux-phone-oco/lab-tools/pull/118))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.5...lpoco-bringup-v0.3.6) - 2026-11-18

### Added

- *(bringup)* initramfs build, a reproducible newc cpio byte for byte as gen_init_cpio ([#114](https://github.com/linux-phone-oco/lab-tools/pull/114))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.4...lpoco-bringup-v0.3.5) - 2026-11-18

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))
- image swap, a control image with parts replaced and its AVB footer rebuilt ([#110](https://github.com/linux-phone-oco/lab-tools/pull/110))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.3.3...lpoco-bringup-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.2.1...lpoco-bringup-v0.2.2) - 2026-11-17

### Added

- --help and -h print the usage in lpoco, lpoco-host, lpoco-bringup and lpoco-health ([#93](https://github.com/linux-phone-oco/lab-tools/pull/93))

## [0.2.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.2.0...lpoco-bringup-v0.2.1) - 2026-11-17

### Other

- install lpoco and bringup from the release tags; wrong-pin tests use 99.0 ([#90](https://github.com/linux-phone-oco/lab-tools/pull/90))

## [0.2.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-bringup-v0.1.7...lpoco-bringup-v0.2.0) - 2026-11-17

### Added

- *(bringup)* follow the family pin, extend by step hooks, library example ([#88](https://github.com/linux-phone-oco/lab-tools/pull/88))
