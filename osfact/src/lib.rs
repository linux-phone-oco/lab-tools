// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact (os-factory): one installable image of a device and the one command that installs it, made from three
//! inputs - the device's declaration (for a phone its device.toml: kernel, firmware, boot format, install steps),
//! a distro profile (its base, package format and package set) and the proven parts - for any distro and any
//! device, developed from phones first (decision 2026-11-27-osfact-own-rust-in-lab-tools, linux-phone-oco/docs).
//!
//! Nothing is built here yet: the crate is the home and its tests. tests/boundary.rs holds the boundary (no lpoco
//! crate in this crate's tree); tests/walleye_two_distros.rs is the failing test the first build makes pass.
