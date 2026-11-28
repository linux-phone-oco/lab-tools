// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact (os-factory): one installable image of a device and the one command that installs it, made from three
//! inputs - the device's declaration (for a phone its device.toml: kernel, firmware, boot format, install steps),
//! a distro profile (its base, package format and package set) and the proven parts - for any distro and any
//! device, developed from phones first (decision 2026-11-27-osfact-own-rust-in-lab-tools, in the project docs).
//!
//! Built so far: `install` (an install.toml read, its commands listed). tests/boundary.rs holds the boundary (no lpoco
//! crate in this crate's tree); tests/no_names_in_source.rs holds the seams (no distro or device name in src/); the first-pack test is the ignored one in tests/.
pub mod install;
