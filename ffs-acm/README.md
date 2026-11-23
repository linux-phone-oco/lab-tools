# ffs-acm - lpoco-ffs-acm: helpers the phone runs

A Rust crate of the lpoco family; how to use it is in the repository README, its changes in CHANGELOG.md.

- `ffs-acm`: a serial console for a phone through FunctionFS (one class-2 interface, a pty shell bridged to the bulk endpoints).
- `restart2 --command bootloader [--dry-run]`: syncs, then restarts the phone through reboot(2) RESTART2 with `bootloader`
  as its argument. busybox's `reboot bootloader` drops the argument (busybox signals init, init calls reboot(RB_AUTOBOOT)),
  so the phone restarts into Linux, not its bootloader. Only `bootloader` is accepted. A device declares it in
  `device.toml` as `[boot.to_bootloader]` (`tool` an `[images.NAME]` with a sha256, `to` where it lands on the phone,
  `run` the command); build it static for the phone's arch (aarch64-unknown-linux-musl). Lifted from
  google-walleye-lab `tools/walleye-reboot`.
