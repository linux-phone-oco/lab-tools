# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.18.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.18.1...lpoco-v0.18.2) - 2026-11-18

### Added

- *(manifest)* a bad [boot.gadget] word is refused when device.toml is read ([#185](https://github.com/linux-phone-oco/lab-tools/pull/185))

## [0.18.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.18.0...lpoco-v0.18.1) - 2026-11-18

### Other

- *(health)* mark the keep-awake as a workaround for sxmo-autosuspend-backoff ([#181](https://github.com/linux-phone-oco/lab-tools/pull/181))

## [0.18.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.17.1...lpoco-v0.18.0) - 2026-11-18

### Added

- *(manifest)* device.toml declared once in Rust, unknown fields refused, schema projected ([#179](https://github.com/linux-phone-oco/lab-tools/pull/179))

## [0.17.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.17.0...lpoco-v0.17.1) - 2026-11-18

### Fixed

- *(host)* restore the per-device phone share ([#164](https://github.com/linux-phone-oco/lab-tools/pull/164)) and the 0.14.2 changelogs that #165 reverted ([#177](https://github.com/linux-phone-oco/lab-tools/pull/177))

## [0.17.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.16.0...lpoco-v0.17.0) - 2026-11-18

### Added

- *(host)* a passing share check records the rule it proved, so the first plan asks no sudo ([#175](https://github.com/linux-phone-oco/lab-tools/pull/175))

## [0.16.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.15.1...lpoco-v0.16.0) - 2026-11-18

### Added

- *(host)* record each root file prepare installs, so plan reads an unreadable one as up to date ([#173](https://github.com/linux-phone-oco/lab-tools/pull/173))

## [0.15.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.15.0...lpoco-v0.15.1) - 2026-11-18

### Added

- *(lab)* a health run names a phone-side piece off main, from the report it reads back ([#170](https://github.com/linux-phone-oco/lab-tools/pull/170))

## [0.15.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.14.1...lpoco-v0.15.0) - 2026-11-18

### Added

- *(host)* check names a stale phone-side piece from the phone's last gathered report ([#165](https://github.com/linux-phone-oco/lab-tools/pull/165))

## [0.14.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.14.0...lpoco-v0.14.1) - 2026-11-18

### Fixed

- *(host)* prepare installs tools by writing a new file and renaming it over the old ([#162](https://github.com/linux-phone-oco/lab-tools/pull/162))

## [0.14.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.13.0...lpoco-v0.14.0) - 2026-11-18

### Added

- lpoco-host check exercises the share, prepare ends with it, revoke now removes the grant ([#160](https://github.com/linux-phone-oco/lab-tools/pull/160))

## [0.13.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.12.2...lpoco-v0.13.0) - 2026-11-18

### Added

- root is asked only by prepare behind sudo; the phone share is a polkit-started unit ([#159](https://github.com/linux-phone-oco/lab-tools/pull/159))
- lpoco-lab flash bakes the image without root or a mount (debugfs and mtools) ([#156](https://github.com/linux-phone-oco/lab-tools/pull/156))

## [0.12.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.12.1...lpoco-v0.12.2) - 2026-11-18

### Added

- *(health)* lab-pieces check reports each declared file's blob id and package version ([#150](https://github.com/linux-phone-oco/lab-tools/pull/150))
- lpoco-lab job: a long job is its own user unit that holds the lab awake while it runs ([#153](https://github.com/linux-phone-oco/lab-tools/pull/153))

### Fixed

- devlab watch no longer files a board task: the verdict is a journal line ([#152](https://github.com/linux-phone-oco/lab-tools/pull/152))

## [0.12.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.12.0...lpoco-v0.12.1) - 2026-11-18

### Added

- *(health)* lpoco-health awake, the lab phone's keep-awake as a package unit ([#148](https://github.com/linux-phone-oco/lab-tools/pull/148))

## [0.12.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.11.0...lpoco-v0.12.0) - 2026-11-18

### Fixed

- *(host)* start an enabled watcher that is stopped ([#146](https://github.com/linux-phone-oco/lab-tools/pull/146))

## [0.11.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.10.0...lpoco-v0.11.0) - 2026-11-18

### Added

- *(host)* gadget net_address - the host's link config for the named USB network ([#144](https://github.com/linux-phone-oco/lab-tools/pull/144))

## [0.10.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.9.0...lpoco-v0.10.0) - 2026-11-18

### Added

- *(host)* gadget net_name - a stable interface name by serial ([#142](https://github.com/linux-phone-oco/lab-tools/pull/142))

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.8.0...lpoco-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.7.1...lpoco-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.7.0...lpoco-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.6.1...lpoco-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.6.0...lpoco-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.5.0...lpoco-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.4.1...lpoco-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.4.0...lpoco-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.8...lpoco-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.7...lpoco-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.6...lpoco-v0.3.7) - 2026-11-18

### Fixed

- *(release)* release-plz ignores its own release commits, so a release PR opens no next one ([#119](https://github.com/linux-phone-oco/lab-tools/pull/119))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.5...lpoco-v0.3.6) - 2026-11-18

### Fixed

- *(release)* a changelog may complete a version its package already carries ([#116](https://github.com/linux-phone-oco/lab-tools/pull/116))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.4...lpoco-v0.3.5) - 2026-11-18

### Fixed

- *(release)* every commit of a pull request names its kind, and a changelog never changes alone ([#112](https://github.com/linux-phone-oco/lab-tools/pull/112))

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.3...lpoco-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.3.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.3.0...lpoco-v0.3.1) - 2026-11-18

### Other

- the mark and the health service say lpoco, kept so by a test (names-sweep) ([#100](https://github.com/linux-phone-oco/lab-tools/pull/100))
- README commands use the lpoco names, kept so by a test (names-sweep) ([#99](https://github.com/linux-phone-oco/lab-tools/pull/99))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.2.1...lpoco-v0.2.2) - 2026-11-17

### Added

- --help and -h print the usage in lpoco, lpoco-host, lpoco-bringup and lpoco-health ([#93](https://github.com/linux-phone-oco/lab-tools/pull/93))

## [0.2.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.2.0...lpoco-v0.2.1) - 2026-11-17

### Other

- install lpoco and bringup from the release tags; wrong-pin tests use 99.0 ([#90](https://github.com/linux-phone-oco/lab-tools/pull/90))

## [0.2.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-v0.1.7...lpoco-v0.2.0) - 2026-11-17

### Added

- lpoco mcp, one MCP server for the family, old tool names kept ([#87](https://github.com/linux-phone-oco/lab-tools/pull/87))
