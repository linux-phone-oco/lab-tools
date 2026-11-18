# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
