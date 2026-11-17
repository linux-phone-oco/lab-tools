# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
