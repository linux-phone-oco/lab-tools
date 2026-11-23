# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.46.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.46.2...lpoco-tone-v0.46.3) - 2026-11-23

### Added

- *(bringup)* image ensure - an image by sha256 from Nix's store, a cache or its recipe ([#368](https://github.com/linux-phone-oco/lab-tools/pull/368))

## [0.46.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.46.1...lpoco-tone-v0.46.2) - 2026-11-23

### Added

- *(manifest)* [boot.to_bootloader] and restart2, a RESTART2 helper for the way to the bootloader ([#358](https://github.com/linux-phone-oco/lab-tools/pull/358))

## [0.46.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.46.0...lpoco-tone-v0.46.1) - 2026-11-23

### Added

- *(host)* follow installs a moved host-only tool without waiting for a keeper's lease ([#355](https://github.com/linux-phone-oco/lab-tools/pull/355))

## [0.46.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.45.0...lpoco-tone-v0.46.0) - 2026-11-23

### Added

- *(host)* prepare writes the user timers a release-pinned tool ships ([#353](https://github.com/linux-phone-oco/lab-tools/pull/353))

## [0.45.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.8...lpoco-tone-v0.45.0) - 2026-11-23

### Added

- *(bringup)* kernel assemble takes --reproduces and archive/ branches ([#351](https://github.com/linux-phone-oco/lab-tools/pull/351))

## [0.44.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.7...lpoco-tone-v0.44.8) - 2026-11-23

### Added

- *(host)* follow installs a declared tool whose lab.toml pin moved ([#348](https://github.com/linux-phone-oco/lab-tools/pull/348))

## [0.44.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.6...lpoco-tone-v0.44.7) - 2026-11-23

### Other

- *(host)* follow builds a release that has no binaries with cargo ([#346](https://github.com/linux-phone-oco/lab-tools/pull/346))

## [0.44.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.5...lpoco-tone-v0.44.6) - 2026-11-23

### Added

- *(lpoco)* lpoco-manifest is tagged only when manifest/ changes ([#344](https://github.com/linux-phone-oco/lab-tools/pull/344))

## [0.44.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.4...lpoco-tone-v0.44.5) - 2026-11-23

### Added

- *(lpoco)* lpoco-attended is tagged only when it changes ([#342](https://github.com/linux-phone-oco/lab-tools/pull/342))

## [0.44.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.3...lpoco-tone-v0.44.4) - 2026-11-23

### Added

- *(host)* follow does not retry a failed release before every has passed ([#340](https://github.com/linux-phone-oco/lab-tools/pull/340))

## [0.44.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.2...lpoco-tone-v0.44.3) - 2026-11-23

### Fixed

- *(layer)* boot.img plugin finds Debian's initrd; prove Debian through a real dpkg -i ([#339](https://github.com/linux-phone-oco/lab-tools/pull/339))

## [0.44.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.1...lpoco-tone-v0.44.2) - 2026-11-23

### Added

- *(layer)* boot.img trigger per base for android-bootimg layers ([#335](https://github.com/linux-phone-oco/lab-tools/pull/335))

## [0.44.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.44.0...lpoco-tone-v0.44.1) - 2026-11-23

### Added

- *(host)* follow holds every phone's keeper lease while it installs ([#333](https://github.com/linux-phone-oco/lab-tools/pull/333))

## [0.44.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.43.0...lpoco-tone-v0.44.0) - 2026-11-23

### Added

- *(host)* [tools.NAME] installs another project's release binary at its pin, checked by its sha256 ([#331](https://github.com/linux-phone-oco/lab-tools/pull/331))

## [0.43.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.42.2...lpoco-tone-v0.43.0) - 2026-11-23

### Added

- *(bringup)* kernel assemble and publish: record checks, then rhizoid assemble ([#330](https://github.com/linux-phone-oco/lab-tools/pull/330))

### Other

- a map at the top of the README and a README per crate ([#328](https://github.com/linux-phone-oco/lab-tools/pull/328))

## [0.42.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.42.1...lpoco-tone-v0.42.2) - 2026-11-21

### Fixed

- *(host)* follow judges the installed lpoco against the pin ([#326](https://github.com/linux-phone-oco/lab-tools/pull/326))

## [0.42.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.42.0...lpoco-tone-v0.42.1) - 2026-11-21

### Fixed

- *(devlab)* keeper release hardened: no-op when nothing held, tombstone first ([#324](https://github.com/linux-phone-oco/lab-tools/pull/324))

## [0.42.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.41.1...lpoco-tone-v0.42.0) - 2026-11-21

### Added

- *(host)* follow installs only inside the [follow] window ([#321](https://github.com/linux-phone-oco/lab-tools/pull/321))

### Fixed

- *(devlab)* keeper release finds its lease by name from any folder ([#323](https://github.com/linux-phone-oco/lab-tools/pull/323))

## [0.41.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.41.0...lpoco-tone-v0.41.1) - 2026-11-21

### Fixed

- *(host)* follow judges only FAIL lines of a check ([#319](https://github.com/linux-phone-oco/lab-tools/pull/319))

## [0.41.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.40.0...lpoco-tone-v0.41.0) - 2026-11-20

### Added

- *(attended)* boot_once runs declared [kernels] pre_boot gates before fastboot boot ([#317](https://github.com/linux-phone-oco/lab-tools/pull/317))

## [0.40.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.39.2...lpoco-tone-v0.40.0) - 2026-11-20

### Added

- *(devlab)* [connection] login = key reaches the phone by ssh -i its declared identity ([#316](https://github.com/linux-phone-oco/lab-tools/pull/316))

## [0.39.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.39.1...lpoco-tone-v0.39.2) - 2026-11-20

### Added

- *(devlab)* a device with no login finds the suite commands not applicable ([#312](https://github.com/linux-phone-oco/lab-tools/pull/312))

## [0.39.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.39.0...lpoco-tone-v0.39.1) - 2026-11-20

### Added

- *(attended)* boot_once checks the image's sha256 and the gadget's serial ([#310](https://github.com/linux-phone-oco/lab-tools/pull/310))

## [0.39.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.38.1...lpoco-tone-v0.39.0) - 2026-11-20

### Added

- *(attended)* boot_once boots by fastboot from memory where the device says so ([#308](https://github.com/linux-phone-oco/lab-tools/pull/308))

## [0.38.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.38.0...lpoco-tone-v0.38.1) - 2026-11-19

### Fixed

- *(attended)* EOF on stdin is no answer; with no terminal a prompt waits on its answer file ([#306](https://github.com/linux-phone-oco/lab-tools/pull/306))

## [0.38.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.37.3...lpoco-tone-v0.38.0) - 2026-11-19

### Added

- *(lab)* the phone is reached by its declared transport (ssh, telnet, adb) and login ([#304](https://github.com/linux-phone-oco/lab-tools/pull/304))

## [0.37.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.37.2...lpoco-tone-v0.37.3) - 2026-11-19

### Added

- *(attended)* a settling group settles the way backs of finished groups; leave_known_good is one ([#302](https://github.com/linux-phone-oco/lab-tools/pull/302))

## [0.37.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.37.1...lpoco-tone-v0.37.2) - 2026-11-19

### Added

- *(attended)* named step groups with a way back, phone_ready and leave_known_good ([#300](https://github.com/linux-phone-oco/lab-tools/pull/300))

## [0.37.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.37.0...lpoco-tone-v0.37.1) - 2026-11-19

### Added

- *(host)* a follow killed mid-install is stepped back by the next run ([#298](https://github.com/linux-phone-oco/lab-tools/pull/298))

## [0.37.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.36.0...lpoco-tone-v0.37.0) - 2026-11-19

### Added

- *(host)* the follow and clones services read the lab they were prepared from ([#296](https://github.com/linux-phone-oco/lab-tools/pull/296))

## [0.36.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.35.1...lpoco-tone-v0.36.0) - 2026-11-19

### Added

- *(host)* follow and prepare take one flock, so two runs never change the host together ([#293](https://github.com/linux-phone-oco/lab-tools/pull/293))

### Fixed

- *(host)* clones moves only commons places, names any other ([#295](https://github.com/linux-phone-oco/lab-tools/pull/295))

## [0.35.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.35.0...lpoco-tone-v0.35.1) - 2026-11-19

### Added

- *(lab)* a try that no new boot answers reports the host's USB timeline for the phone ([#291](https://github.com/linux-phone-oco/lab-tools/pull/291))

## [0.35.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.34.2...lpoco-tone-v0.35.0) - 2026-11-19

### Added

- *(host)* lpoco host clones fast-forwards the shared clones from a user timer ([#289](https://github.com/linux-phone-oco/lab-tools/pull/289))

## [0.34.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.34.1...lpoco-tone-v0.34.2) - 2026-11-19

### Added

- *(lab)* lpoco-lab manifest check reads a device.toml as the host does, writing nothing ([#287](https://github.com/linux-phone-oco/lab-tools/pull/287))

## [0.34.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.34.0...lpoco-tone-v0.34.1) - 2026-11-19

### Added

- *(attended)* boot_once and back_to_known_good skip when the kernel already runs ([#286](https://github.com/linux-phone-oco/lab-tools/pull/286))
- *(attended)* run --from STEP skips the steps before it; an --until stop names its --from ([#285](https://github.com/linux-phone-oco/lab-tools/pull/285))
- *(attended)* a prompt is filed and its answer file read, the terminal answering too ([#284](https://github.com/linux-phone-oco/lab-tools/pull/284))
- *(attended)* lpoco attended lift flags near-duplicate steps across device repos by body ([#282](https://github.com/linux-phone-oco/lab-tools/pull/282))

## [0.34.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.33.0...lpoco-tone-v0.34.0) - 2026-11-19

### Added

- *(host)* lpoco host follow installs a newer release in a lease-free window and lands its pin ([#280](https://github.com/linux-phone-oco/lab-tools/pull/280))

## [0.33.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.32.2...lpoco-tone-v0.33.0) - 2026-11-19

### Added

- *(host)* install the family's programs from release assets, hash checked, cargo as the fallback ([#278](https://github.com/linux-phone-oco/lab-tools/pull/278))

## [0.32.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.32.1...lpoco-tone-v0.32.2) - 2026-11-19

### Added

- *(attended)* check --host runs the installed devlab on the manifest ([#276](https://github.com/linux-phone-oco/lab-tools/pull/276))

## [0.32.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.32.0...lpoco-tone-v0.32.1) - 2026-11-19

### Added

- *(devlab)* kernel try --cmdline boots an installed kernel once with extra options ([#272](https://github.com/linux-phone-oco/lab-tools/pull/272))
- *(attended)* ship lpoco-attended with the family and refuse a stray word ([#274](https://github.com/linux-phone-oco/lab-tools/pull/274))
- *(attended)* an optional per-call time limit (--call-limit SECONDS) ([#275](https://github.com/linux-phone-oco/lab-tools/pull/275))

## [0.32.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.31.1...lpoco-tone-v0.32.0) - 2026-11-19

### Added

- *(manifest)* homes for the attended keys; lpoco-attended reads dev() and proc() ([#270](https://github.com/linux-phone-oco/lab-tools/pull/270))

## [0.31.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.31.0...lpoco-tone-v0.31.1) - 2026-11-19

### Added

- *(attended)* lpoco-attended new|run|check for a device repo's procedures ([#268](https://github.com/linux-phone-oco/lab-tools/pull/268))

## [0.31.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.30.0...lpoco-tone-v0.31.0) - 2026-11-19

### Added

- *(attended)* boot_once and back_to_known_good read the device's [kernels] ([#267](https://github.com/linux-phone-oco/lab-tools/pull/267))
- *(attended)* physical acts over a Bench, the person by default ([#265](https://github.com/linux-phone-oco/lab-tools/pull/265))

## [0.30.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.6...lpoco-tone-v0.30.0) - 2026-11-19

### Added

- *(attended)* wait_for_gadget reads the usb_id the device declares; [[host.gadget]] usb_id ([#264](https://github.com/linux-phone-oco/lab-tools/pull/264))
- *(attended)* the log files what the phone said to a passing step ([#262](https://github.com/linux-phone-oco/lab-tools/pull/262))

## [0.29.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.5...lpoco-tone-v0.29.6) - 2026-11-19

### Added

- *(attended)* procedure() loads the named device's device.toml and hands devlab that file ([#260](https://github.com/linux-phone-oco/lab-tools/pull/260))

## [0.29.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.4...lpoco-tone-v0.29.5) - 2026-11-19

### Added

- *(health)* suspend cycle --before/--after hooks ([#257](https://github.com/linux-phone-oco/lab-tools/pull/257))
- *(attended)* --until FIRST_PROMPT and --part NAME ([#259](https://github.com/linux-phone-oco/lab-tools/pull/259))

## [0.29.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.3...lpoco-tone-v0.29.4) - 2026-11-19

### Added

- *(attended)* wait_for_usb is keyed to bring-up step gadget-enumerates ([#255](https://github.com/linux-phone-oco/lab-tools/pull/255))

## [0.29.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.2...lpoco-tone-v0.29.3) - 2026-11-19

### Added

- *(attended)* library steps boot_once and back_to_known_good, tested on a stand-in ([#253](https://github.com/linux-phone-oco/lab-tools/pull/253))

## [0.29.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.1...lpoco-tone-v0.29.2) - 2026-11-19

### Added

- *(attended)* library steps read_dmesg and wait_for_usb, tested on a stand-in and a fake bus ([#251](https://github.com/linux-phone-oco/lab-tools/pull/251))

## [0.29.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.29.0...lpoco-tone-v0.29.1) - 2026-11-19

### Added

- *(attended)* library steps dmesg_clean, check_probe, wait_for_phone, tested on a stand-in ([#250](https://github.com/linux-phone-oco/lab-tools/pull/250))

### Other

- *(attended)* the example names the release that carries it ([#248](https://github.com/linux-phone-oco/lab-tools/pull/248))

## [0.29.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.28.3...lpoco-tone-v0.29.0) - 2026-11-19

### Added

- *(attended)* lpoco-attended, a lane's attended procedure as free functions over one Step type ([#247](https://github.com/linux-phone-oco/lab-tools/pull/247))

### Other

- *(bringup)* the attended example names the release that carries it ([#245](https://github.com/linux-phone-oco/lab-tools/pull/245))

## [0.28.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.28.2...lpoco-tone-v0.28.3) - 2026-11-19

### Added

- *(bringup)* attended, a library for a lane's attended procedure as a list of named steps ([#243](https://github.com/linux-phone-oco/lab-tools/pull/243))

## [0.28.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.28.1...lpoco-tone-v0.28.2) - 2026-11-19

### Other

- *(bringup)* the bisect test decodes the kernel frame without the dtb behind it ([#241](https://github.com/linux-phone-oco/lab-tools/pull/241))

## [0.28.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.28.0...lpoco-tone-v0.28.1) - 2026-11-19

### Other

- *(devlab)* the hold test locks a folder of its own, not the lab user's state folder ([#239](https://github.com/linux-phone-oco/lab-tools/pull/239))

## [0.28.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.27.3...lpoco-tone-v0.28.0) - 2026-11-19

### Added

- *(layer)* apk also writes the cmdline to /usr/lib/kernel-cmdline.d/50-NAME.conf ([#237](https://github.com/linux-phone-oco/lab-tools/pull/237))

## [0.27.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.27.2...lpoco-tone-v0.27.3) - 2026-11-19

### Added

- *(layer)* apk gets Nura's /etc/deviceinfo from [boot] and [device] ([#235](https://github.com/linux-phone-oco/lab-tools/pull/235)) ([#235](https://github.com/linux-phone-oco/lab-tools/pull/235))

## [0.27.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.27.1...lpoco-tone-v0.27.2) - 2026-11-19

### Added

- *(layer)* refuse a modules/ symlink that leaves the module tree ([#233](https://github.com/linux-phone-oco/lab-tools/pull/233))

## [0.27.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.27.0...lpoco-tone-v0.27.1) - 2026-11-19

### Added

- *(layer)* apk -boot package with mkinitfs module globs and a feature scriptlet ([#231](https://github.com/linux-phone-oco/lab-tools/pull/231))

## [0.27.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.26.1...lpoco-tone-v0.27.0) - 2026-11-19

### Added

- *(layer)* --kernel-build packs a finished kernel build as linux-NAME for arm64 ([#229](https://github.com/linux-phone-oco/lab-tools/pull/229))

## [0.26.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.26.0...lpoco-tone-v0.26.1) - 2026-11-19

### Other

- *(layer)* pack_run asserts the written postinstall is the one nfpm is given ([#227](https://github.com/linux-phone-oco/lab-tools/pull/227))

## [0.26.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.25.0...lpoco-tone-v0.26.0) - 2026-11-19

### Added

- *(layer)* [layer.names.FORMAT] names a dependency token per package format ([#225](https://github.com/linux-phone-oco/lab-tools/pull/225))

## [0.25.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.24.3...lpoco-tone-v0.25.0) - 2026-11-19

### Added

- *(layer)* lpoco layer pack applies the systemd preset after install, on first install only ([#223](https://github.com/linux-phone-oco/lab-tools/pull/223))

## [0.24.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.24.2...lpoco-tone-v0.24.3) - 2026-11-19

### Other

- lpoco layer pack run against a stand-in nfpm ([#221](https://github.com/linux-phone-oco/lab-tools/pull/221))

## [0.24.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.24.1...lpoco-tone-v0.24.2) - 2026-11-19

### Added

- *(layer)* lpoco layer pack refuses an unpackable layer and removes stale packages ([#219](https://github.com/linux-phone-oco/lab-tools/pull/219))

## [0.24.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.24.0...lpoco-tone-v0.24.1) - 2026-11-19

### Added

- *(layer)* lpoco layer pack - the device layer as nfpm packages for apk, deb, rpm and archlinux ([#217](https://github.com/linux-phone-oco/lab-tools/pull/217))

## [0.24.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.23.0...lpoco-tone-v0.24.0) - 2026-11-19

### Added

- *(manifest)* [layer] table - files, dependency tokens, kernel, boot, firmware, early modules ([#212](https://github.com/linux-phone-oco/lab-tools/pull/212))

## [0.23.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.22.2...lpoco-tone-v0.23.0) - 2026-11-19

### Added

- *(health)* suspend cycle stops and restarts [health] suspend_stop services ([#209](https://github.com/linux-phone-oco/lab-tools/pull/209))

## [0.22.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.22.1...lpoco-tone-v0.22.2) - 2026-11-19

### Added

- *(health)* test-layer-quiet base check, and each report says which layer the phone booted ([#207](https://github.com/linux-phone-oco/lab-tools/pull/207))

## [0.22.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.22.0...lpoco-tone-v0.22.1) - 2026-11-19

### Fixed

- *(bringup)* dt-check's verdict comes from the schema's output, defconfig first, tools baked in ([#205](https://github.com/linux-phone-oco/lab-tools/pull/205))

### Other

- MIT OR Apache-2.0 with LICENSE texts, SPDX headers, REUSE.toml and a licence check ([#203](https://github.com/linux-phone-oco/lab-tools/pull/203))

## [0.22.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.21.2...lpoco-tone-v0.22.0) - 2026-11-19

### Added

- *(host)* name a phone user config that drifts from its default or base copy ([#201](https://github.com/linux-phone-oco/lab-tools/pull/201))

## [0.21.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.21.1...lpoco-tone-v0.21.2) - 2026-11-19

### Added

- *(bringup)* dt-check binding|dtbs, the kernel's devicetree checks in a digest-pinned container ([#199](https://github.com/linux-phone-oco/lab-tools/pull/199))

## [0.21.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.21.0...lpoco-tone-v0.21.1) - 2026-11-19

### Added

- *(bringup)* reference capture and diff, the stock OS against our boot ([#197](https://github.com/linux-phone-oco/lab-tools/pull/197))

## [0.21.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.20.1...lpoco-tone-v0.21.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] console = "ffs" makes the init start the FunctionFS ACM helper ([#195](https://github.com/linux-phone-oco/lab-tools/pull/195))

### Other

- *(ffs-acm)* license MIT OR Apache-2.0, like the rest of the lpoco family ([#194](https://github.com/linux-phone-oco/lab-tools/pull/194))

## [0.20.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.20.0...lpoco-tone-v0.20.1) - 2026-11-18

### Added

- *(ffs-acm)* the FunctionFS ACM console helper joins the family ([#192](https://github.com/linux-phone-oco/lab-tools/pull/192))

## [0.20.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.19.0...lpoco-tone-v0.20.0) - 2026-11-18

### Other

- *(manifest)* ShellWord - a gadget word the init cannot carry cannot be held ([#190](https://github.com/linux-phone-oco/lab-tools/pull/190))

## [0.19.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.18.3...lpoco-tone-v0.19.0) - 2026-11-18

### Added

- *(bringup)* [boot.gadget] window - a read-only log LUN in the init ([#188](https://github.com/linux-phone-oco/lab-tools/pull/188))

## [0.18.3](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.18.2...lpoco-tone-v0.18.3) - 2026-11-18

### Added

- *(bringup)* initramfs files - the init's DHCP and selftest files shipped with it ([#186](https://github.com/linux-phone-oco/lab-tools/pull/186))

## [0.18.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.18.1...lpoco-tone-v0.18.2) - 2026-11-18

### Added

- *(manifest)* a bad [boot.gadget] word is refused when device.toml is read ([#185](https://github.com/linux-phone-oco/lab-tools/pull/185))

## [0.18.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.18.0...lpoco-tone-v0.18.1) - 2026-11-18

### Other

- *(health)* mark the keep-awake as a workaround for sxmo-autosuspend-backoff ([#181](https://github.com/linux-phone-oco/lab-tools/pull/181))

## [0.18.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.17.1...lpoco-tone-v0.18.0) - 2026-11-18

### Added

- *(manifest)* device.toml declared once in Rust, unknown fields refused, schema projected ([#179](https://github.com/linux-phone-oco/lab-tools/pull/179))

## [0.17.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.17.0...lpoco-tone-v0.17.1) - 2026-11-18

### Fixed

- *(host)* restore the per-device phone share ([#164](https://github.com/linux-phone-oco/lab-tools/pull/164)) and the 0.14.2 changelogs that #165 reverted ([#177](https://github.com/linux-phone-oco/lab-tools/pull/177))

## [0.17.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.16.0...lpoco-tone-v0.17.0) - 2026-11-18

### Added

- *(host)* a passing share check records the rule it proved, so the first plan asks no sudo ([#175](https://github.com/linux-phone-oco/lab-tools/pull/175))

## [0.16.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.15.1...lpoco-tone-v0.16.0) - 2026-11-18

### Added

- *(host)* record each root file prepare installs, so plan reads an unreadable one as up to date ([#173](https://github.com/linux-phone-oco/lab-tools/pull/173))

## [0.15.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.15.0...lpoco-tone-v0.15.1) - 2026-11-18

### Added

- *(lab)* a health run names a phone-side piece off main, from the report it reads back ([#170](https://github.com/linux-phone-oco/lab-tools/pull/170))

## [0.15.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.14.1...lpoco-tone-v0.15.0) - 2026-11-18

### Added

- *(host)* check names a stale phone-side piece from the phone's last gathered report ([#165](https://github.com/linux-phone-oco/lab-tools/pull/165))

## [0.14.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.14.0...lpoco-tone-v0.14.1) - 2026-11-18

### Fixed

- *(host)* prepare installs tools by writing a new file and renaming it over the old ([#162](https://github.com/linux-phone-oco/lab-tools/pull/162))

## [0.14.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.13.0...lpoco-tone-v0.14.0) - 2026-11-18

### Added

- lpoco-host check exercises the share, prepare ends with it, revoke now removes the grant ([#160](https://github.com/linux-phone-oco/lab-tools/pull/160))

## [0.13.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.12.2...lpoco-tone-v0.13.0) - 2026-11-18

### Added

- root is asked only by prepare behind sudo; the phone share is a polkit-started unit ([#159](https://github.com/linux-phone-oco/lab-tools/pull/159))
- lpoco-lab flash bakes the image without root or a mount (debugfs and mtools) ([#156](https://github.com/linux-phone-oco/lab-tools/pull/156))

## [0.12.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.12.1...lpoco-tone-v0.12.2) - 2026-11-18

### Added

- *(health)* lab-pieces check reports each declared file's blob id and package version ([#150](https://github.com/linux-phone-oco/lab-tools/pull/150))
- lpoco-lab job: a long job is its own user unit that holds the lab awake while it runs ([#153](https://github.com/linux-phone-oco/lab-tools/pull/153))

### Fixed

- devlab watch no longer files a board task: the verdict is a journal line ([#152](https://github.com/linux-phone-oco/lab-tools/pull/152))

## [0.12.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.12.0...lpoco-tone-v0.12.1) - 2026-11-18

### Added

- *(health)* lpoco-health awake, the lab phone's keep-awake as a package unit ([#148](https://github.com/linux-phone-oco/lab-tools/pull/148))

## [0.12.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.11.0...lpoco-tone-v0.12.0) - 2026-11-18

### Fixed

- *(host)* start an enabled watcher that is stopped ([#146](https://github.com/linux-phone-oco/lab-tools/pull/146))

## [0.11.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.10.0...lpoco-tone-v0.11.0) - 2026-11-18

### Added

- *(host)* gadget net_address - the host's link config for the named USB network ([#144](https://github.com/linux-phone-oco/lab-tools/pull/144))

## [0.10.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.9.0...lpoco-tone-v0.10.0) - 2026-11-18

### Added

- *(host)* gadget net_name - a stable interface name by serial ([#142](https://github.com/linux-phone-oco/lab-tools/pull/142))

## [0.9.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.8.0...lpoco-tone-v0.9.0) - 2026-11-18

### Added

- *(host)* [host] targets - plan names each cross target, check fails a missing one ([#140](https://github.com/linux-phone-oco/lab-tools/pull/140))

## [0.8.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.7.1...lpoco-tone-v0.8.0) - 2026-11-18

### Added

- *(host)* [host] commands - plan names each, check fails a missing one, nothing installed ([#138](https://github.com/linux-phone-oco/lab-tools/pull/138))

## [0.7.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.7.0...lpoco-tone-v0.7.1) - 2026-11-18

### Added

- *(host)* plan and check name foreign udev rules that could cross ours ([#136](https://github.com/linux-phone-oco/lab-tools/pull/136))

## [0.7.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.6.1...lpoco-tone-v0.7.0) - 2026-11-18

### Added

- *(host)* watchers only for devices whose [host] says watch = true ([#135](https://github.com/linux-phone-oco/lab-tools/pull/135))
- *(host)* devlab-watch@ user units per device, enabled and restarted by prepare ([#133](https://github.com/linux-phone-oco/lab-tools/pull/133))

## [0.6.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.6.0...lpoco-tone-v0.6.1) - 2026-11-18

### Added

- *(host)* check names a gadget that enumerated with a serial no device declares ([#131](https://github.com/linux-phone-oco/lab-tools/pull/131))

## [0.6.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.5.0...lpoco-tone-v0.6.0) - 2026-11-18

### Added

- *(host)* gadget_group in lab.toml gives each gadget node a group and mode 0660 ([#129](https://github.com/linux-phone-oco/lab-tools/pull/129))

## [0.5.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.4.1...lpoco-tone-v0.5.0) - 2026-11-18

### Fixed

- *(host)* lpoco host reads lab.toml from origin/main, naming the rev ([#127](https://github.com/linux-phone-oco/lab-tools/pull/127))

## [0.4.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.4.0...lpoco-tone-v0.4.1) - 2026-11-18

### Added

- *(health)* lpoco-health suspend plan|cycle, one controlled suspend counted ([#125](https://github.com/linux-phone-oco/lab-tools/pull/125))

## [0.4.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.8...lpoco-tone-v0.4.0) - 2026-11-18

### Added

- *(bringup)* image bisect SERIES, a declared series of swaps counted against the control ([#123](https://github.com/linux-phone-oco/lab-tools/pull/123))

## [0.3.8](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.7...lpoco-tone-v0.3.8) - 2026-11-18

### Fixed

- *(release)* every member lists the others in changelog_include, one PR writes all changelogs ([#121](https://github.com/linux-phone-oco/lab-tools/pull/121))

### Other

- release v0.3.7 ([#120](https://github.com/linux-phone-oco/lab-tools/pull/120))

## [0.3.7](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.6...lpoco-tone-v0.3.7) - 2026-11-18

### Other

- release v0.3.6 ([#117](https://github.com/linux-phone-oco/lab-tools/pull/117))

## [0.3.6](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.5...lpoco-tone-v0.3.6) - 2026-11-18

### Other

- release v0.3.6 ([#115](https://github.com/linux-phone-oco/lab-tools/pull/115))

## [0.3.5](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.4...lpoco-tone-v0.3.5) - 2026-11-18

### Other

- release v0.3.4 ([#109](https://github.com/linux-phone-oco/lab-tools/pull/109))

## [0.3.4](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.3...lpoco-tone-v0.3.4) - 2026-11-18

### Other

- release v0.3.4 ([#107](https://github.com/linux-phone-oco/lab-tools/pull/107))

## [0.3.1](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.3.0...lpoco-tone-v0.3.1) - 2026-11-18

### Other

- lab tools builder/lpoco tone argenv ([#101](https://github.com/linux-phone-oco/lab-tools/pull/101))

## [0.3.0](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.2.2...lpoco-tone-v0.3.0) - 2026-11-18

### Other

- release v0.2.2 ([#94](https://github.com/linux-phone-oco/lab-tools/pull/94))

## [0.2.2](https://github.com/linux-phone-oco/lab-tools/compare/lpoco-tone-v0.2.1...lpoco-tone-v0.2.2) - 2026-11-17

### Other

- release v0.2.2 ([#92](https://github.com/linux-phone-oco/lab-tools/pull/92))
