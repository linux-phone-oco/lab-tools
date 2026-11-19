# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.27.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.27.0...lpoco-manifest-v0.27.1) - 2026-11-19

### Added

- *(layer)* apk -boot package with mkinitfs module globs and a feature scriptlet ([#231](https://github.com/linux-phone-oco/lab-tools/pull/231))

## [0.27.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.26.1...lpoco-manifest-v0.27.0) - 2026-11-19

### Added

- *(layer)* --kernel-build packs a finished kernel build as linux-NAME for arm64 ([#229](https://github.com/linux-phone-oco/lab-tools/pull/229))

## [0.26.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.26.0...lpoco-manifest-v0.26.1) - 2026-11-19

### Other

- *(layer)* pack_run asserts the written postinstall is the one nfpm is given ([#227](https://github.com/linux-phone-oco/lab-tools/pull/227))

## [0.26.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.25.0...lpoco-manifest-v0.26.0) - 2026-11-19

### Added

- *(layer)* [layer.names.FORMAT] names a dependency token per package format ([#225](https://github.com/linux-phone-oco/lab-tools/pull/225))

## [0.25.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.24.3...lpoco-manifest-v0.25.0) - 2026-11-19

### Added

- *(layer)* lpoco layer pack applies the systemd preset after install, on first install only ([#223](https://github.com/linux-phone-oco/lab-tools/pull/223))

## [0.24.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.24.2...lpoco-manifest-v0.24.3) - 2026-11-19

### Other

- lpoco layer pack run against a stand-in nfpm ([#221](https://github.com/linux-phone-oco/lab-tools/pull/221))

## [0.24.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.24.1...lpoco-manifest-v0.24.2) - 2026-11-19

### Added

- *(layer)* lpoco layer pack refuses an unpackable layer and removes stale packages ([#219](https://github.com/linux-phone-oco/lab-tools/pull/219))

## [0.24.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.24.0...lpoco-manifest-v0.24.1) - 2026-11-19

### Added

- *(layer)* lpoco layer pack - the device layer as nfpm packages for apk, deb, rpm and archlinux ([#217](https://github.com/linux-phone-oco/lab-tools/pull/217))

## [0.24.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.23.0...lpoco-manifest-v0.24.0) - 2026-11-19

### Added

- *(manifest)* [layer] table - files, dependency tokens, kernel, boot, firmware, early modules ([#212](https://github.com/linux-phone-oco/lab-tools/pull/212))

## [0.23.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.22.2...lpoco-manifest-v0.23.0) - 2026-11-19

### Added

- *(health)* suspend cycle stops and restarts [health] suspend_stop services ([#209](https://github.com/linux-phone-oco/lab-tools/pull/209))

## [0.22.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.22.1...lpoco-manifest-v0.22.2) - 2026-11-19

### Added

- *(health)* test-layer-quiet base check, and each report says which layer the phone booted ([#207](https://github.com/linux-phone-oco/lab-tools/pull/207))

## [0.22.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.22.0...lpoco-manifest-v0.22.1) - 2026-11-19

### Fixed

- *(bringup)* dt-check's verdict comes from the schema's output, defconfig first, tools baked in ([#205](https://github.com/linux-phone-oco/lab-tools/pull/205))

### Other

- MIT OR Apache-2.0 with LICENSE texts, SPDX headers, REUSE.toml and a licence check ([#203](https://github.com/linux-phone-oco/lab-tools/pull/203))

## [0.22.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.21.2...lpoco-manifest-v0.22.0) - 2026-11-19

### Added

- *(host)* name a phone user config that drifts from its default or base copy ([#201](https://github.com/linux-phone-oco/lab-tools/pull/201))

## [0.21.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.21.1...lpoco-manifest-v0.21.2) - 2026-11-19

### Added

- *(bringup)* dt-check binding|dtbs, the kernel's devicetree checks in a digest-pinned container ([#199](https://github.com/linux-phone-oco/lab-tools/pull/199))

## [0.21.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.21.0...lpoco-manifest-v0.21.1) - 2026-11-19

### Added

- *(bringup)* reference capture and diff, the stock OS against our boot ([#197](https://github.com/linux-phone-oco/lab-tools/pull/197))

## [0.21.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.20.1...lpoco-manifest-v0.21.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] console = "ffs" makes the init start the FunctionFS ACM helper ([#195](https://github.com/linux-phone-oco/lab-tools/pull/195))

### Other

- *(ffs-acm)* license MIT OR Apache-2.0, like the rest of the lpoco family ([#194](https://github.com/linux-phone-oco/lab-tools/pull/194))

## [0.20.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.20.0...lpoco-manifest-v0.20.1) - 2026-11-18

### Added

- *(ffs-acm)* the FunctionFS ACM console helper joins the family ([#192](https://github.com/linux-phone-oco/lab-tools/pull/192))

## [0.20.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.19.0...lpoco-manifest-v0.20.0) - 2026-11-18

### Other

- *(manifest)* ShellWord - a gadget word the init cannot carry cannot be held ([#190](https://github.com/linux-phone-oco/lab-tools/pull/190))

## [0.19.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.18.3...lpoco-manifest-v0.19.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] window - a read-only log LUN in the init ([#188](https://github.com/linux-phone-oco/lab-tools/pull/188))

## [0.18.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.18.2...lpoco-manifest-v0.18.3) - 2026-11-18

### Added

- *(bringup)* initramfs files - the init's DHCP and selftest files shipped with it ([#186](https://github.com/linux-phone-oco/lab-tools/pull/186))

## [0.18.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.18.1...lpoco-manifest-v0.18.2) - 2026-11-18

### Added

- *(manifest)* a bad [boot.gadget] word is refused when device.toml is read ([#185](https://github.com/linux-phone-oco/lab-tools/pull/185))

## [0.18.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.18.0...lpoco-manifest-v0.18.1) - 2026-11-18

### Other

- *(health)* mark the keep-awake as a workaround for sxmo-autosuspend-backoff ([#181](https://github.com/linux-phone-oco/lab-tools/pull/181))

## [0.18.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.17.1...lpoco-manifest-v0.18.0) - 2026-11-18

### Added

- *(manifest)* device.toml declared once in Rust, unknown fields refused, schema projected ([#179](https://github.com/linux-phone-oco/lab-tools/pull/179))

## [0.17.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-manifest-v0.17.0...lpoco-manifest-v0.17.1) - 2026-11-18

### Added

- device.toml declared once: every table and field, unknown ones refused by devlab, host and bringup alike, its JSON Schema (draft-07) projected and committed
