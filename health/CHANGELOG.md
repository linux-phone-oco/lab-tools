# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.27.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.26.1...lpoco-health-v0.27.0) - 2026-11-19

### Added

- *(layer)* --kernel-build packs a finished kernel build as linux-NAME for arm64 ([#229](https://github.com/linux-phone-oco/lab-tools/pull/229))

## [0.26.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.26.0...lpoco-health-v0.26.1) - 2026-11-19

### Other

- *(layer)* pack_run asserts the written postinstall is the one nfpm is given ([#227](https://github.com/linux-phone-oco/lab-tools/pull/227))

## [0.26.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.25.0...lpoco-health-v0.26.0) - 2026-11-19

### Added

- *(layer)* [layer.names.FORMAT] names a dependency token per package format ([#225](https://github.com/linux-phone-oco/lab-tools/pull/225))

## [0.25.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.24.3...lpoco-health-v0.25.0) - 2026-11-19

### Added

- *(layer)* lpoco layer pack applies the systemd preset after install, on first install only ([#223](https://github.com/linux-phone-oco/lab-tools/pull/223))

## [0.24.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.24.2...lpoco-health-v0.24.3) - 2026-11-19

### Other

- lpoco layer pack run against a stand-in nfpm ([#221](https://github.com/linux-phone-oco/lab-tools/pull/221))

## [0.24.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.24.1...lpoco-health-v0.24.2) - 2026-11-19

### Added

- *(layer)* lpoco layer pack refuses an unpackable layer and removes stale packages ([#219](https://github.com/linux-phone-oco/lab-tools/pull/219))

## [0.24.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.24.0...lpoco-health-v0.24.1) - 2026-11-19

### Added

- *(layer)* lpoco layer pack - the device layer as nfpm packages for apk, deb, rpm and archlinux ([#217](https://github.com/linux-phone-oco/lab-tools/pull/217))

## [0.24.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.23.0...lpoco-health-v0.24.0) - 2026-11-19

### Added

- *(manifest)* [layer] table - files, dependency tokens, kernel, boot, firmware, early modules ([#212](https://github.com/linux-phone-oco/lab-tools/pull/212))

## [0.23.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.22.2...lpoco-health-v0.23.0) - 2026-11-19

### Added

- *(health)* suspend cycle stops and restarts [health] suspend_stop services ([#209](https://github.com/linux-phone-oco/lab-tools/pull/209))

## [0.22.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.22.1...lpoco-health-v0.22.2) - 2026-11-19

### Added

- *(health)* test-layer-quiet base check, and each report says which layer the phone booted ([#207](https://github.com/linux-phone-oco/lab-tools/pull/207))

## [0.22.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.22.0...lpoco-health-v0.22.1) - 2026-11-19

### Fixed

- *(bringup)* dt-check's verdict comes from the schema's output, defconfig first, tools baked in ([#205](https://github.com/linux-phone-oco/lab-tools/pull/205))

### Other

- MIT OR Apache-2.0 with LICENSE texts, SPDX headers, REUSE.toml and a licence check ([#203](https://github.com/linux-phone-oco/lab-tools/pull/203))

## [0.22.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.21.2...lpoco-health-v0.22.0) - 2026-11-19

### Added

- *(host)* name a phone user config that drifts from its default or base copy ([#201](https://github.com/linux-phone-oco/lab-tools/pull/201))

## [0.21.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.21.1...lpoco-health-v0.21.2) - 2026-11-19

### Added

- *(bringup)* dt-check binding|dtbs, the kernel's devicetree checks in a digest-pinned container ([#199](https://github.com/linux-phone-oco/lab-tools/pull/199))

## [0.21.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.21.0...lpoco-health-v0.21.1) - 2026-11-19

### Added

- *(bringup)* reference capture and diff, the stock OS against our boot ([#197](https://github.com/linux-phone-oco/lab-tools/pull/197))

## [0.21.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.20.1...lpoco-health-v0.21.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] console = "ffs" makes the init start the FunctionFS ACM helper ([#195](https://github.com/linux-phone-oco/lab-tools/pull/195))

### Other

- *(ffs-acm)* license MIT OR Apache-2.0, like the rest of the lpoco family ([#194](https://github.com/linux-phone-oco/lab-tools/pull/194))

## [0.20.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.20.0...lpoco-health-v0.20.1) - 2026-11-18

### Added

- *(ffs-acm)* the FunctionFS ACM console helper joins the family ([#192](https://github.com/linux-phone-oco/lab-tools/pull/192))

## [0.20.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.19.0...lpoco-health-v0.20.0) - 2026-11-18

### Other

- *(manifest)* ShellWord - a gadget word the init cannot carry cannot be held ([#190](https://github.com/linux-phone-oco/lab-tools/pull/190))

## [0.19.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.18.3...lpoco-health-v0.19.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] window - a read-only log LUN in the init ([#188](https://github.com/linux-phone-oco/lab-tools/pull/188))

## [0.18.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.18.2...lpoco-health-v0.18.3) - 2026-11-18

### Added

- *(bringup)* initramfs files - the init's DHCP and selftest files shipped with it ([#186](https://github.com/linux-phone-oco/lab-tools/pull/186))

## [0.18.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.18.1...lpoco-health-v0.18.2) - 2026-11-18

### Added

- *(manifest)* a bad [boot.gadget] word is refused when device.toml is read ([#185](https://github.com/linux-phone-oco/lab-tools/pull/185))

## [0.18.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.18.0...lpoco-health-v0.18.1) - 2026-11-18

### Other

- *(health)* mark the keep-awake as a workaround for sxmo-autosuspend-backoff ([#181](https://github.com/linux-phone-oco/lab-tools/pull/181))

## [0.18.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.17.1...lpoco-health-v0.18.0) - 2026-11-18

### Added

- *(manifest)* device.toml declared once in Rust, unknown fields refused, schema projected ([#179](https://github.com/linux-phone-oco/lab-tools/pull/179))

## [0.17.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.17.0...lpoco-health-v0.17.1) - 2026-11-18

### Fixed

- *(host)* restore the per-device phone share ([#164](https://github.com/linux-phone-oco/lab-tools/pull/164)) and the 0.14.2 changelogs that #165 reverted ([#177](https://github.com/linux-phone-oco/lab-tools/pull/177))

## [0.17.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.16.0...lpoco-health-v0.17.0) - 2026-11-18

### Added

- *(host)* a passing share check records the rule it proved, so the first plan asks no sudo ([#175](https://github.com/linux-phone-oco/lab-tools/pull/175))

## [0.16.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.15.1...lpoco-health-v0.16.0) - 2026-11-18

### Added

- *(host)* record each root file prepare installs, so plan reads an unreadable one as up to date ([#173](https://github.com/linux-phone-oco/lab-tools/pull/173))

## [0.15.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.15.0...lpoco-health-v0.15.1) - 2026-11-18

### Added

- *(lab)* a health run names a phone-side piece off main, from the report it reads back ([#170](https://github.com/linux-phone-oco/lab-tools/pull/170))

## [0.15.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.14.1...lpoco-health-v0.15.0) - 2026-11-18

### Added

- *(host)* check names a stale phone-side piece from the phone's last gathered report ([#165](https://github.com/linux-phone-oco/lab-tools/pull/165))

## [0.14.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.14.0...lpoco-health-v0.14.1) - 2026-11-18

### Fixed

- *(host)* prepare installs tools by writing a new file and renaming it over the old ([#162](https://github.com/linux-phone-oco/lab-tools/pull/162))

## [0.14.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.13.0...lpoco-health-v0.14.0) - 2026-11-18

### Added

- lpoco-host check exercises the share, prepare ends with it, revoke now removes the grant ([#160](https://github.com/linux-phone-oco/lab-tools/pull/160))

## [0.13.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.12.2...lpoco-health-v0.13.0) - 2026-11-18

### Added

- root is asked only by prepare behind sudo; the phone share is a polkit-started unit ([#159](https://github.com/linux-phone-oco/lab-tools/pull/159))
- lpoco-lab flash bakes the image without root or a mount (debugfs and mtools) ([#156](https://github.com/linux-phone-oco/lab-tools/pull/156))

## [0.12.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.12.1...lpoco-health-v0.12.2) - 2026-11-18

### Added

- *(health)* lab-pieces check reports each declared file's blob id and package version ([#150](https://github.com/linux-phone-oco/lab-tools/pull/150))
- lpoco-lab job: a long job is its own user unit that holds the lab awake while it runs ([#153](https://github.com/linux-phone-oco/lab-tools/pull/153))

### Fixed

- devlab watch no longer files a board task: the verdict is a journal line ([#152](https://github.com/linux-phone-oco/lab-tools/pull/152))

## [0.12.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.12.0...lpoco-health-v0.12.1) - 2026-11-18

### Added

- *(health)* lpoco-health awake, the lab phone's keep-awake as a package unit ([#148](https://github.com/linux-phone-oco/lab-tools/pull/148))

## [0.12.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.11.0...lpoco-health-v0.12.0) - 2026-11-18

### Fixed

- *(host)* start an enabled watcher that is stopped ([#146](https://github.com/linux-phone-oco/lab-tools/pull/146))

## [0.11.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.10.0...lpoco-health-v0.11.0) - 2026-11-18

### Added

- *(host)* gadget net_address - the host's link config for the named USB network ([#144](https://github.com/linux-phone-oco/lab-tools/pull/144))

## [0.10.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.9.0...lpoco-health-v0.10.0) - 2026-11-18

### Added

- *(host)* gadget net_name - a stable interface name by serial ([#142](https://github.com/linux-phone-oco/lab-tools/pull/142))

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-health-v0.8.0...lpoco-health-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

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
