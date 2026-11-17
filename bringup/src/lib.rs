//! lpoco-bringup (library): the host side of bringing a phone up. It makes files on the host; devlab puts
//! them on the phone. First piece: boot images, header v0 (`lpoco bringup image ...`).

pub mod boot;
pub mod fdt;
pub mod image;
