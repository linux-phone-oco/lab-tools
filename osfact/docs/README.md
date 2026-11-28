# osfact: what it is, its boundary, its way out

Decided in linux-phone-oco/docs, decision `2026-11-27-osfact-own-rust-in-lab-tools` (research
`osfact-how-it-stands`, `osfact-image-builders-landscape`); the author's words in session log rows
`2026-11-27-author-image-factory-any-distro` and `2026-11-27-author-os-factory-any-device`.

## What it does

    osfact pack now --device DEVICE.toml --distro PROFILE.toml --parts DIR --out OUT
    -> OUT/rootfs.img, OUT/root.tar.gz (the root tree gzipped, what a RAM install pushes), OUT/boot.img (the device's boot format), OUT/install.toml, and one install command

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
5. **install**: `install.toml` - with `[install.connection]`, `[install.root]` and `[install.way_back]` declared: the boot
   from RAM, the push of `root.tar.gz` (its sha256 checked on the device) and its entry by chroot, nothing written to the
   device; without them the boot alone. Each step a write of one output to one named place by one transport, the
   device's `[install]` with the output's sha256 - and the command that runs it.
   The way back is confirmed, not only sent: `[install.way_back] back_on` is a host command (argv, `{serial}` the
   device's serial) tried until it ends with status 0 within the way back's `wait`; a run whose way back was sent and
   not answered says so, naming it. Every value that reaches a device shell line is one literal word: paths are
   quoted, a command (`receive`, `command`, `check`) is split by shell quoting rules and each word quoted (a `;` or
   `$(...)` in it is an argument), and a value that cannot be quoted (a line break, a NUL, an unclosed quote) does not
   read. The one shell line left is the way back's own `command` (it is a restart line, `sync; echo b > ...`).

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

## Install a release

Each release `osfact-vX.Y.Z` carries the x86_64 binary and its hash (`.github/workflows/assets.yml`):

    gh release download osfact-vX.Y.Z --repo linux-phone-oco/lab-tools --pattern 'osfact-x86_64*' --dir DIR
    (cd DIR && sha256sum -c osfact-x86_64.sha256) && install -m 755 DIR/osfact-x86_64 ~/.local/bin/osfact
    osfact install now --plan install.toml --dry-run

A keeper pins the version and checks the sha256 before it runs the binary.
