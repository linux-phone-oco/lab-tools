// SPDX-License-Identifier: MIT OR Apache-2.0
//! Boot images, header v0: the format is osfact's (its android-bootimg adapter, the boot-format interface); this
//! module hands it the header facts of a device's manifest ([boot], read by lpoco-manifest) and keeps the names
//! bring-up's callers use.

use lpoco_manifest::Boot;
pub use osfact::android_bootimg::{pad_to, read, v0_id, Image, MAGIC};
pub use osfact::boot::{Finding, Verdict};

/// The header facts osfact packs and counts by, taken from the manifest's `[boot]`.
fn facts(boot: &Boot) -> osfact::boot::Boot {
    osfact::boot::Boot {
        format: osfact::android_bootimg::AndroidBootimg::NAME.to_string(),
        header_version: boot.header_version,
        base: boot.base,
        kernel_offset: boot.kernel_offset,
        ramdisk_offset: boot.ramdisk_offset,
        second_offset: boot.second_offset,
        tags_offset: boot.tags_offset,
        pagesize: boot.pagesize,
        os_version: boot.os_version.clone(),
        os_patch_level: boot.os_patch_level.clone(),
        cmdline: boot.cmdline.clone(),
        partition_size: boot.partition_size,
        msm_id: boot.msm_id.clone(),
        // bringup adds the footer itself, and reads the dtb and the waits from the manifest
        dtb: None,
        avb_footer: false,
        back_wait: None,
    }
}

/// The image mkbootimg would write for these parts and the manifest's header facts (no second image).
pub fn pack(boot: &Boot, kernel: &[u8], ramdisk: &[u8]) -> Result<Vec<u8>, String> {
    osfact::android_bootimg::pack(&facts(boot), kernel, ramdisk)
}

/// What a boot image must be for the device whose manifest says `boot`.
pub fn check(bytes: &[u8], boot: &Boot) -> Vec<Finding> {
    osfact::android_bootimg::check(bytes, &facts(boot))
}
