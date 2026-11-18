# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.12.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.12.1...lpoco-tone-v0.12.2) - 2026-11-18

### Added

- *(health)* lab-pieces check reports each declared file's blob id and package version ([#150](https://github.com/linux-phone-oco/lab-tools/pull/150))
- lpoco-lab job: a long job is its own user unit that holds the lab awake while it runs ([#153](https://github.com/linux-phone-oco/lab-tools/pull/153))

### Fixed

- devlab watch no longer files a board task: the verdict is a journal line ([#152](https://github.com/linux-phone-oco/lab-tools/pull/152))

## [0.12.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.12.0...lpoco-tone-v0.12.1) - 2026-11-18

### Added

- *(health)* lpoco-health awake, the lab phone's keep-awake as a package unit ([#148](https://github.com/linux-phone-oco/lab-tools/pull/148))

## [0.12.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.11.0...lpoco-tone-v0.12.0) - 2026-11-18

### Fixed

- *(host)* start an enabled watcher that is stopped ([#146](https://github.com/linux-phone-oco/lab-tools/pull/146))

## [0.11.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.10.0...lpoco-tone-v0.11.0) - 2026-11-18

### Added

- *(host)* gadget net_address - the host's link config for the named USB network ([#144](https://github.com/linux-phone-oco/lab-tools/pull/144))

## [0.10.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.9.0...lpoco-tone-v0.10.0) - 2026-11-18

### Added

- *(host)* gadget net_name - a stable interface name by serial ([#142](https://github.com/linux-phone-oco/lab-tools/pull/142))

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.8.0...lpoco-tone-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.7.1...lpoco-tone-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.7.0...lpoco-tone-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.6.1...lpoco-tone-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.6.0...lpoco-tone-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.5.0...lpoco-tone-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.4.1...lpoco-tone-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.4.0...lpoco-tone-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.8...lpoco-tone-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.7...lpoco-tone-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.6...lpoco-tone-v0.3.7) - 2026-11-18

### Other

- release v0.3.6 ([#117](https://github.com/linux-phone-oco/lab-tools/pull/117))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.5...lpoco-tone-v0.3.6) - 2026-11-18

### Other

- release v0.3.6 ([#115](https://github.com/linux-phone-oco/lab-tools/pull/115))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.4...lpoco-tone-v0.3.5) - 2026-11-18

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.3...lpoco-tone-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.3.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.0...lpoco-tone-v0.3.1) - 2026-11-18

### Other

- lab tools builder/lpoco tone argenv ([#101](https://github.com/linux-phone-oco/lab-tools/pull/101))

## [0.3.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.2.2...lpoco-tone-v0.3.0) - 2026-11-18

### Other

- release v0.2.2 ([#94](https://github.com/linux-phone-oco/lab-tools/pull/94))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.2.1...lpoco-tone-v0.2.2) - 2026-11-17

### Other

- release v0.2.2 ([#92](https://github.com/linux-phone-oco/lab-tools/pull/92))
