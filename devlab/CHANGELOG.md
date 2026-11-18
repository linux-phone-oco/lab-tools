# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.8.0...lpoco-lab-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.7.1...lpoco-lab-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.7.0...lpoco-lab-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.6.1...lpoco-lab-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.6.0...lpoco-lab-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.5.0...lpoco-lab-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.4.1...lpoco-lab-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.4.0...lpoco-lab-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.8...lpoco-lab-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.7...lpoco-lab-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.6...lpoco-lab-v0.3.7) - 2026-11-18

### Other

- release v0.3.6 ([#117](https://github.com/linux-phone-oco/lab-tools/pull/117))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.5...lpoco-lab-v0.3.6) - 2026-11-18

### Other

- release v0.3.6 ([#115](https://github.com/linux-phone-oco/lab-tools/pull/115))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.4...lpoco-lab-v0.3.5) - 2026-11-18

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.3...lpoco-lab-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.3.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.2...lpoco-lab-v0.3.3) - 2026-11-18

### Added

- suite check suspend-irq-quiet, interrupts across one suspended cycle ([#106](https://github.com/linux-phone-oco/lab-tools/pull/106))

## [0.3.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.1...lpoco-lab-v0.3.2) - 2026-11-18

### Added

- lpoco-lab reads its declared environment through the argenv contract ([#103](https://github.com/linux-phone-oco/lab-tools/pull/103))

### Other

- messages tell the reader to type lpoco-lab, kept so by a test (names-sweep) ([#104](https://github.com/linux-phone-oco/lab-tools/pull/104))

## [0.3.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.3.0...lpoco-lab-v0.3.1) - 2026-11-18

### Other

- lpoco lab argenv ([#98](https://github.com/linux-phone-oco/lab-tools/pull/98))

## [0.3.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.2.2...lpoco-lab-v0.3.0) - 2026-11-18

### Other

- release v0.2.2 ([#94](https://github.com/linux-phone-oco/lab-tools/pull/94))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.2.1...lpoco-lab-v0.2.2) - 2026-11-17

### Other

- release v0.2.2 ([#92](https://github.com/linux-phone-oco/lab-tools/pull/92))

## [0.2.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-lab-v0.1.7...lpoco-lab-v0.2.0) - 2026-11-17

### Added

- lpoco mcp, one MCP server for the family, old tool names kept ([#87](https://github.com/linux-phone-oco/lab-tools/pull/87))
