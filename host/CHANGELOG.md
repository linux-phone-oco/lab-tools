# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.14.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.14.1...lpoco-host-v0.14.2) - 2026-11-18

### Fixed

- the phone share is per device: own nft table, forwarding kept until the last stops ([#164](https://github.com/linux-phone-oco/lab-tools/pull/164))

## [0.14.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.14.0...lpoco-host-v0.14.1) - 2026-11-18

### Fixed

- *(host)* prepare installs tools by writing a new file and renaming it over the old ([#162](https://github.com/linux-phone-oco/lab-tools/pull/162))

## [0.14.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.13.0...lpoco-host-v0.14.0) - 2026-11-18

### Added

- lpoco-host check exercises the share, prepare ends with it, revoke now removes the grant ([#160](https://github.com/linux-phone-oco/lab-tools/pull/160))

## [0.13.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.12.2...lpoco-host-v0.13.0) - 2026-11-18

### Added

- root is asked only by prepare behind sudo; the phone share is a polkit-started unit ([#159](https://github.com/linux-phone-oco/lab-tools/pull/159))
- lpoco-lab flash bakes the image without root or a mount (debugfs and mtools) ([#156](https://github.com/linux-phone-oco/lab-tools/pull/156))

## [0.12.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.12.1...lpoco-host-v0.12.2) - 2026-11-18

### Added

- *(health)* lab-pieces check reports each declared file's blob id and package version ([#150](https://github.com/linux-phone-oco/lab-tools/pull/150))
- lpoco-lab job: a long job is its own user unit that holds the lab awake while it runs ([#153](https://github.com/linux-phone-oco/lab-tools/pull/153))

### Fixed

- devlab watch no longer files a board task: the verdict is a journal line ([#152](https://github.com/linux-phone-oco/lab-tools/pull/152))

## [0.12.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.12.0...lpoco-host-v0.12.1) - 2026-11-18

### Added

- *(health)* lpoco-health awake, the lab phone's keep-awake as a package unit ([#148](https://github.com/linux-phone-oco/lab-tools/pull/148))

## [0.12.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.11.0...lpoco-host-v0.12.0) - 2026-11-18

### Fixed

- *(host)* start an enabled watcher that is stopped ([#146](https://github.com/linux-phone-oco/lab-tools/pull/146))

## [0.11.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.10.0...lpoco-host-v0.11.0) - 2026-11-18

### Added

- *(host)* gadget net_address - the host's link config for the named USB network ([#144](https://github.com/linux-phone-oco/lab-tools/pull/144))

## [0.10.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.9.0...lpoco-host-v0.10.0) - 2026-11-18

### Added

- *(host)* gadget net_name - a stable interface name by serial ([#142](https://github.com/linux-phone-oco/lab-tools/pull/142))

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.8.0...lpoco-host-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.7.1...lpoco-host-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-host-v0.7.0...lpoco-host-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

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
