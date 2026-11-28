# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.4.0...osfact-v0.4.1) - 2026-11-28

### Added

- *(manifest)* osfact's tables are read by osfact alone ([#518](https://github.com/linux-phone-oco/lab-tools/pull/518))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.3.0...osfact-v0.4.0) - 2026-11-28

### Added

- *(osfact)* enter mounts the declared kernel filesystems (binds) into the root before the chroot ([#515](https://github.com/linux-phone-oco/lab-tools/pull/515))

## [0.3.0](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.2.0...osfact-v0.3.0) - 2026-11-28

### Added

- *(osfact)* the root gets the links the packages' install scripts would make ([#511](https://github.com/linux-phone-oco/lab-tools/pull/511))

## [0.2.0](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.1.2...osfact-v0.2.0) - 2026-11-28

### Added

- *(osfact)* enter logs the device's text, the dry run lists its lines, check_wait is declared ([#506](https://github.com/linux-phone-oco/lab-tools/pull/506))

## [0.1.2](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.1.1...osfact-v0.1.2) - 2026-11-28

### Fixed

- *(osfact)* the push receiver starts with its input closed, so job control does not stop it ([#501](https://github.com/linux-phone-oco/lab-tools/pull/501))

## [0.1.1](https://github.com/linux-phone-oco/lab-tools/compare/osfact-v0.1.0...osfact-v0.1.1) - 2026-11-28

### Fixed

- *(osfact)* a relative --out gave apk a relative keys dir, so every index was untrusted ([#498](https://github.com/linux-phone-oco/lab-tools/pull/498))

## [0.1.0] - 2026-11-28

### Added

- first release: the walking skeleton (pack, rootfs, install) as a prebuilt x86_64 binary with its sha256

## [0.0.0] - 2026-11-27

### Added

- the home of osfact: no code yet; its boundary test and the failing walleye test (two distros, one declaration)
