# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.42.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.41.1...lpoco-attended-v0.42.0) - 2026-11-21

### Added

- *(host)* follow installs only inside the [follow] window ([#321](https://github.com/linux-phone-oco/lab-tools/pull/321))

### Fixed

- *(devlab)* keeper release finds its lease by name from any folder ([#323](https://github.com/linux-phone-oco/lab-tools/pull/323))

## [0.41.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.41.0...lpoco-attended-v0.41.1) - 2026-11-21

### Fixed

- *(host)* follow judges only FAIL lines of a check ([#319](https://github.com/linux-phone-oco/lab-tools/pull/319))

## [0.41.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.40.0...lpoco-attended-v0.41.0) - 2026-11-20

### Added

- *(attended)* boot_once runs declared [kernels] pre_boot gates before fastboot boot ([#317](https://github.com/linux-phone-oco/lab-tools/pull/317))

## [0.40.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.39.2...lpoco-attended-v0.40.0) - 2026-11-20

### Added

- *(devlab)* [connection] login = key reaches the phone by ssh -i its declared identity ([#316](https://github.com/linux-phone-oco/lab-tools/pull/316))

## [0.39.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.39.1...lpoco-attended-v0.39.2) - 2026-11-20

### Added

- *(devlab)* a device with no login finds the suite commands not applicable ([#312](https://github.com/linux-phone-oco/lab-tools/pull/312))

## [0.39.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.39.0...lpoco-attended-v0.39.1) - 2026-11-20

### Added

- *(attended)* boot_once checks the image's sha256 and the gadget's serial ([#310](https://github.com/linux-phone-oco/lab-tools/pull/310))

## [0.39.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.38.1...lpoco-attended-v0.39.0) - 2026-11-20

### Added

- *(attended)* boot_once boots by fastboot from memory where the device says so ([#308](https://github.com/linux-phone-oco/lab-tools/pull/308))

## [0.38.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.38.0...lpoco-attended-v0.38.1) - 2026-11-19

### Fixed

- *(attended)* EOF on stdin is no answer; with no terminal a prompt waits on its answer file ([#306](https://github.com/linux-phone-oco/lab-tools/pull/306))

## [0.38.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.37.3...lpoco-attended-v0.38.0) - 2026-11-19

### Added

- *(lab)* the phone is reached by its declared transport (ssh, telnet, adb) and login ([#304](https://github.com/linux-phone-oco/lab-tools/pull/304))

## [0.37.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.37.2...lpoco-attended-v0.37.3) - 2026-11-19

### Added

- *(attended)* a settling group settles the way backs of finished groups; leave_known_good is one ([#302](https://github.com/linux-phone-oco/lab-tools/pull/302))

## [0.37.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.37.1...lpoco-attended-v0.37.2) - 2026-11-19

### Added

- *(attended)* named step groups with a way back, phone_ready and leave_known_good ([#300](https://github.com/linux-phone-oco/lab-tools/pull/300))

## [0.37.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.37.0...lpoco-attended-v0.37.1) - 2026-11-19

### Added

- *(host)* a follow killed mid-install is stepped back by the next run ([#298](https://github.com/linux-phone-oco/lab-tools/pull/298))

## [0.37.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.36.0...lpoco-attended-v0.37.0) - 2026-11-19

### Added

- *(host)* the follow and clones services read the lab they were prepared from ([#296](https://github.com/linux-phone-oco/lab-tools/pull/296))

## [0.36.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.35.1...lpoco-attended-v0.36.0) - 2026-11-19

### Added

- *(host)* follow and prepare take one flock, so two runs never change the host together ([#293](https://github.com/linux-phone-oco/lab-tools/pull/293))

### Fixed

- *(host)* clones moves only commons places, names any other ([#295](https://github.com/linux-phone-oco/lab-tools/pull/295))

## [0.35.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.35.0...lpoco-attended-v0.35.1) - 2026-11-19

### Added

- *(lab)* a try that no new boot answers reports the host's USB timeline for the phone ([#291](https://github.com/linux-phone-oco/lab-tools/pull/291))

## [0.35.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.34.2...lpoco-attended-v0.35.0) - 2026-11-19

### Added

- *(host)* lpoco host clones fast-forwards the shared clones from a user timer ([#289](https://github.com/linux-phone-oco/lab-tools/pull/289))

## [0.34.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.34.1...lpoco-attended-v0.34.2) - 2026-11-19

### Added

- *(lab)* lpoco-lab manifest check reads a device.toml as the host does, writing nothing ([#287](https://github.com/linux-phone-oco/lab-tools/pull/287))

## [0.34.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.34.0...lpoco-attended-v0.34.1) - 2026-11-19

### Added

- *(attended)* boot_once and back_to_known_good skip when the kernel already runs ([#286](https://github.com/linux-phone-oco/lab-tools/pull/286))
- *(attended)* run --from STEP skips the steps before it; an --until stop names its --from ([#285](https://github.com/linux-phone-oco/lab-tools/pull/285))
- *(attended)* a prompt is filed and its answer file read, the terminal answering too ([#284](https://github.com/linux-phone-oco/lab-tools/pull/284))
- *(attended)* lpoco attended lift flags near-duplicate steps across device repos by body ([#282](https://github.com/linux-phone-oco/lab-tools/pull/282))

## [0.34.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.33.0...lpoco-attended-v0.34.0) - 2026-11-19

### Added

- *(host)* lpoco host follow installs a newer release in a lease-free window and lands its pin ([#280](https://github.com/linux-phone-oco/lab-tools/pull/280))

## [0.33.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.32.2...lpoco-attended-v0.33.0) - 2026-11-19

### Added

- *(host)* install the family's programs from release assets, hash checked, cargo as the fallback ([#278](https://github.com/linux-phone-oco/lab-tools/pull/278))

## [0.32.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.32.1...lpoco-attended-v0.32.2) - 2026-11-19

### Added

- *(attended)* check --host runs the installed devlab on the manifest ([#276](https://github.com/linux-phone-oco/lab-tools/pull/276))

## [0.32.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-attended-v0.32.0...lpoco-attended-v0.32.1) - 2026-11-19

### Added

- *(attended)* an optional per-call time limit (--call-limit SECONDS) ([#275](https://github.com/linux-phone-oco/lab-tools/pull/275))
- *(attended)* ship lpoco-attended with the family and refuse a stray word ([#274](https://github.com/linux-phone-oco/lab-tools/pull/274))
- *(devlab)* kernel try --cmdline boots an installed kernel once with extra options ([#272](https://github.com/linux-phone-oco/lab-tools/pull/272))

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
