# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.7.1...lpoco-health-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.7.0...lpoco-health-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.6.1...lpoco-health-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.6.0...lpoco-health-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.5.0...lpoco-health-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.4.1...lpoco-health-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.4.0...lpoco-health-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.8...lpoco-health-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.7...lpoco-health-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.6...lpoco-health-v0.3.7) - 2026-11-18

### Other

- release v0.3.6 ([#117](https://github.com/linux-phone-oco/lab-tools/pull/117))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.5...lpoco-health-v0.3.6) - 2026-11-18

### Other

- release v0.3.6 ([#115](https://github.com/linux-phone-oco/lab-tools/pull/115))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.4...lpoco-health-v0.3.5) - 2026-11-18

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.3...lpoco-health-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.3.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.2...lpoco-health-v0.3.3) - 2026-11-18

### Added

- suite check suspend-irq-quiet, interrupts across one suspended cycle ([#106](https://github.com/linux-phone-oco/lab-tools/pull/106))

## [0.3.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.3.0...lpoco-health-v0.3.1) - 2026-11-18

### Other

- the mark and the health service say lpoco, kept so by a test (names-sweep) ([#100](https://github.com/linux-phone-oco/lab-tools/pull/100))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.2.1...lpoco-health-v0.2.2) - 2026-11-17

### Added

- --help and -h print the usage in lpoco, lpoco-host, lpoco-bringup and lpoco-health ([#93](https://github.com/linux-phone-oco/lab-tools/pull/93))

## [0.2.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.1.7...lpoco-health-v0.2.0) - 2026-11-17

### Added

- devlab becomes lpoco-lab and tone lpoco-tone, old names kept as second binaries ([#86](https://github.com/linux-phone-oco/lab-tools/pull/86))

### Other

- lpoco dispatcher ([#83](https://github.com/linux-phone-oco/lab-tools/pull/83))

## [0.1.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.1.6...lpoco-health-v0.1.7) - 2026-11-17

### Fixed

- *(lpoco-health)* keep no_sim and the old library path for the phone's own checks ([#76](https://github.com/linux-phone-oco/lab-tools/pull/76))

## [0.1.6](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.5...devlab-health-v0.1.6) - 2026-11-17

### Added

- *(health)* no_sim helper; mobile data and modem-registered skip 'no SIM card' ([#73](https://github.com/linux-phone-oco/lab-tools/pull/73))

## [0.1.5](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.4...devlab-health-v0.1.5) - 2026-11-17

### Added

- sensors counted through iio-sensor-proxy too; proximity-reads ([#68](https://github.com/linux-phone-oco/lab-tools/pull/68))

## [0.1.4](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.3...devlab-health-v0.1.4) - 2026-11-17

### Fixed

- mobile-data measures the data bearer, never the IMS one beside it ([#62](https://github.com/linux-phone-oco/lab-tools/pull/62))

## [0.1.3](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.2...devlab-health-v0.1.3) - 2026-11-17

### Added

- mobile-data in the generic suite, its bytes counted against a monthly budget ([#60](https://github.com/linux-phone-oco/lab-tools/pull/60))

## [0.1.2](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.1...devlab-health-v0.1.2) - 2026-11-17

### Added

- devlab-health tone make|find: the tone crate on the phone, for checks that listen ([#57](https://github.com/linux-phone-oco/lab-tools/pull/57))

## [0.1.1](https://github.com/linux-phone-oco/lab-tools/compare/devlab-health-v0.1.0...devlab-health-v0.1.1) - 2026-11-17

### Added

- the telephony layer: checks that cost something, at most once a day ([#56](https://github.com/linux-phone-oco/lab-tools/pull/56))

### Other

- *(devlab-health)* release v0.1.0 ([#49](https://github.com/linux-phone-oco/lab-tools/pull/49))

## [0.1.0](https://github.com/linux-phone-oco/lab-tools/releases/tag/devlab-health-v0.1.0) - 2026-11-17

### Added

- Deep sleep's check: soc-sleeps reads the SoC's own counters after every boot ([#39](https://github.com/linux-phone-oco/lab-tools/pull/39))
- The phone checks its own health after every boot: devlab-health and devlab suite ([#29](https://github.com/linux-phone-oco/lab-tools/pull/29))

### Fixed

- Test sounds are quiet: a level and volume per phone, -40 dBFS at 25% by default ([#45](https://github.com/linux-phone-oco/lab-tools/pull/45))
