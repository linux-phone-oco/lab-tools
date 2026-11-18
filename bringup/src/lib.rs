//! lpoco-bringup (library): the host side of bringing a phone up. It makes files on the host; devlab puts
//! them on the phone.
//!
//! A device repository can use these parts from a rust-script step (see `examples/step.rs`):
//!
//! - [`boot::Boot`]: the `[boot]` table of a device manifest, read and checked;
//! - [`image::pack`]: kernel and ramdisk to a header-v0 image, byte for byte as mkbootimg writes it;
//! - [`image::read`], [`image::check`]: an image read back, and counted against the manifest;
//! - [`pin::check`]: stop when the manifest pins another family version;
//! - [`hooks::run`]: call a manifest-named step executable.

pub mod bisect;
pub mod boot;
pub mod fdt;
pub mod hooks;
pub mod image;
pub mod initramfs;
pub mod pin;
pub mod swap;
