// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact (os-factory): one installable image of a device and the one command that installs it, made from three
//! inputs - the device's declaration (for a phone its device.toml: kernel, firmware, boot format, install steps),
//! a distro profile (its base, package format and package set) and the proven parts - for any distro and any
//! device, developed from phones first (decision 2026-11-27-osfact-own-rust-in-lab-tools, in the project docs).
//!
//! Built so far: `pack` (the three stages joined for one device and one distro), `boot` (the boot-format interface and its android-bootimg adapter, with avb and fdt), `install` (an install.toml read, its commands listed, and run: boot from RAM, push the root compressed, enter it; `device` is the real device it runs on), `rootfs` (a distro profile to a rootless root tree and an ext4 image, built twice and compared; `pkgformat` is its interface, `apk` its first adapter, `distro` the profile). tests/boundary.rs holds the boundary (no lpoco
//! crate in this crate's tree); tests/no_names_in_source.rs holds the seams (no distro or device name in src/); the first-pack test is the ignored one in tests/.
pub mod android_bootimg;
pub mod apk;
pub mod avb;
pub mod boot;
pub mod declaration;
pub mod device;
pub mod distro;
pub mod fdt;
pub mod install;
pub mod log;
pub mod pack;
pub mod pkgformat;
pub mod rootfs;
