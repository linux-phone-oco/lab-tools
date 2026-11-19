# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.32.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.31.1...lpoco-attended-v0.32.0) - 2026-11-19

### Added

- *(manifest)* homes for the attended keys; lpoco-attended reads dev() and proc() ([#270](https://github.com/linux-phone-oco/lab-tools/pull/270))

## [0.31.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.31.0...lpoco-attended-v0.31.1) - 2026-11-19

### Added

- *(attended)* lpoco-attended new|run|check for a device repo's procedures ([#268](https://github.com/linux-phone-oco/lab-tools/pull/268))

## [0.31.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.30.0...lpoco-attended-v0.31.0) - 2026-11-19

### Added

- *(attended)* boot_once and back_to_known_good read the device's [kernels] ([#267](https://github.com/linux-phone-oco/lab-tools/pull/267))
- *(attended)* physical acts over a Bench, the person by default ([#265](https://github.com/linux-phone-oco/lab-tools/pull/265))

## [0.30.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.6...lpoco-attended-v0.30.0) - 2026-11-19

### Added

- *(attended)* wait_for_gadget reads the usb_id the device declares; [[host.gadget]] usb_id ([#264](https://github.com/linux-phone-oco/lab-tools/pull/264))
- *(attended)* the log files what the phone said to a passing step ([#262](https://github.com/linux-phone-oco/lab-tools/pull/262))

## [0.29.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.5...lpoco-attended-v0.29.6) - 2026-11-19

### Added

- *(attended)* procedure() loads the named device's device.toml and hands devlab that file ([#260](https://github.com/linux-phone-oco/lab-tools/pull/260))

## [0.29.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.4...lpoco-attended-v0.29.5) - 2026-11-19

### Added

- *(attended)* --until FIRST_PROMPT and --part NAME ([#259](https://github.com/linux-phone-oco/lab-tools/pull/259))
- *(health)* suspend cycle --before/--after hooks ([#257](https://github.com/linux-phone-oco/lab-tools/pull/257))

## [0.29.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.3...lpoco-attended-v0.29.4) - 2026-11-19

### Added

- *(attended)* wait_for_usb is keyed to bring-up step gadget-enumerates ([#255](https://github.com/linux-phone-oco/lab-tools/pull/255))

## [0.29.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.2...lpoco-attended-v0.29.3) - 2026-11-19

### Added

- *(attended)* library steps boot_once and back_to_known_good, tested on a stand-in ([#253](https://github.com/linux-phone-oco/lab-tools/pull/253))

## [0.29.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.1...lpoco-attended-v0.29.2) - 2026-11-19

### Added

- *(attended)* library steps read_dmesg and wait_for_usb, tested on a stand-in and a fake bus ([#251](https://github.com/linux-phone-oco/lab-tools/pull/251))

## [0.29.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.29.0...lpoco-attended-v0.29.1) - 2026-11-19

### Added

- *(attended)* library steps dmesg_clean, check_probe, wait_for_phone, tested on a stand-in ([#250](https://github.com/linux-phone-oco/lab-tools/pull/250))

### Other

- *(attended)* the example names the release that carries it ([#248](https://github.com/linux-phone-oco/lab-tools/pull/248))

## [0.29.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.28.3...lpoco-attended-v0.29.0) - 2026-11-19

### Added

- *(attended)* lpoco-attended, a lane's attended procedure as free functions over one Step type ([#247](https://github.com/linux-phone-oco/lab-tools/pull/247))

## [0.28.3](https://github.com/linux-phone-oco/lab-tools/releases/tag/lpoco-attended-v0.28.3) - 2026-11-19

### Added

- the first slice: procedure, step, check, ask_author, phone, retry and the one Step type, with --dry-run and a log
