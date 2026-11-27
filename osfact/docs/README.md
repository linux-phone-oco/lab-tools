# osfact: what it is, its boundary, its way out

Decided in linux-phone-oco/docs, decision `2026-11-27-osfact-own-rust-in-lab-tools` (research
`osfact-how-it-stands`, `osfact-image-builders-landscape`); the author's words in session log rows
`2026-11-27-author-image-factory-any-distro` and `2026-11-27-author-os-factory-any-device`.

## What it does

    osfact pack now --device DEVICE.toml --distro PROFILE.toml --parts DIR --out OUT
    -> OUT/rootfs.img, OUT/boot.img (the device's boot format), OUT/install.toml, and one install command

Three inputs, one output:

- **the device's declaration** - the tables osfact reads of a device.toml: `[layer]` (kernel, dtbs, firmware,
  initramfs modules, files), `[boot]` (the boot format and its values), `[install]` (the steps that put an image on
  the device). The lab's own tables (connections, procedures, health) are not osfact's and it ignores them.
- **a distro profile** - data, not code: the base's name, its package format (apk, deb, rpm, archlinux), its
  repositories and keys, its init and its package set. No distro is named in osfact's code; a format is.
- **the proven parts** - the packages and files that have passed on the device: the device layer as packages
  of the profile's format, the kernel, the part builds, each by its sha256.

Its stages, each a format- or device-neutral step with one adapter per format or per boot format:

1. **root tree**: the format's own installer, run rootless into a directory - `apk --root --initdb --usermode`
   (apk.static), `mmdebstrap --mode=unshare`, `pacman --root`, `dnf5 --installroot` - with the profile's set and
   the parts in one transaction from a local repository of the parts;
2. **glue**: what the skipped install scripts would have done, written by osfact per format (service links,
   inittab or presets);
3. **file system**: `mke2fs -d` under fakeroot, fixed times and UUID, built twice and compared;
4. **boot artifact**: the device's boot format - `android-bootimg` (header, appended dtb, AVB footer), `efi`;
5. **install**: `install.toml` - each step a write of one output to one named place by one transport, the
   device's `[install]` with the output's sha256 - and the command that runs it.

## The boundary

osfact depends on no lpoco crate, in any dependency section (`tests/boundary.rs` reads `cargo tree`). What it
needs of today's lpoco crates moves into it and lpoco takes it from here: the device-neutral tables of
lpoco-manifest (`[layer]`, `[boot]`), lpoco-layer's per-format packing, lpoco-bringup's boot image and AVB
writers. Lab work stays in lpoco: reaching a device, its keeper's lease, attended procedures (a lab's install
procedure reads osfact's `install.toml`), Nix fixed-output caching (`lpoco bringup ensure` wraps an osfact build).

## Its way out: when osfact leaves for grenudi/os-factory

It moves to its own repository, history kept (`git filter-repo --path osfact/`), when one of these is true:

- a second user outside linux-phone-oco builds an image with it (another project, a device that is not ours);
- a device that is not a phone is served by it (a board, a laptop), so its users are no longer lab-tools' users;
- the author says so.

Until then it lives here to use lab-tools' conveyor (CI, release-plz, land, the records) instead of a second one.
