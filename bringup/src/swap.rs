// SPDX-License-Identifier: MIT OR Apache-2.0
//! Image swap: a control image with one part replaced - ramdisk, kernel, device tree or command line - the
//! header's other facts kept and the AVB hash footer rebuilt over the new image (written by [`avb`]: algorithm NONE, the control's partition size, name, salt and properties). The result says which regions differ from the control.

use crate::avb;
use crate::fdt;
use crate::image::{self, pad_to, v0_id};
use abootimg_oxide::Header;
use std::fs;

/// The parts to put in; `None` keeps the control's.
#[derive(Default)]
pub struct Parts {
    pub kernel: Option<Vec<u8>>,
    pub dtb: Option<Vec<u8>>,
    pub ramdisk: Option<Vec<u8>>,
    pub cmdline: Option<String>,
    /// Leave the AVB footer and the partition padding off (the shape of an image dumped before signing).
    pub no_footer: bool,
}

pub(crate) fn scratch(what: &str) -> Result<std::path::PathBuf, String> {
    let dir =
        std::env::temp_dir().join(format!("lpoco-bringup-swap-{}-{what}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    Ok(dir)
}

/// The control with `parts` swapped in, and a line per region that differs from the control.
pub fn swap(control: &[u8], parts: &Parts) -> Result<(Vec<u8>, Vec<String>), String> {
    let footer = avb::read(control)?;
    let body = match &footer {
        Some(f) => &control[..f.original],
        None => control,
    };
    let img = image::read(body)?;
    let mut h = img.header.clone();
    // the kernel blob is the kernel with its device tree appended: either may be swapped alone
    let (old_kernel, old_dtb) = match fdt::appended(img.kernel) {
        Some((at, _)) => img.kernel.split_at(at),
        None => (img.kernel, &[][..]),
    };
    let mut kernel = parts.kernel.clone().unwrap_or_else(|| old_kernel.to_vec());
    kernel.extend_from_slice(parts.dtb.as_deref().unwrap_or(old_dtb));
    let ramdisk = parts
        .ramdisk
        .clone()
        .unwrap_or_else(|| img.ramdisk.to_vec());
    h.kernel_size = kernel.len() as u32;
    h.ramdisk_size = ramdisk.len() as u32;
    h.hash_digest = v0_id(&kernel, &ramdisk, img.second);
    if let Some(c) = &parts.cmdline {
        if c.contains('\0') {
            return Err("the command line has a NUL byte".into());
        }
        if c.len() > 1535 {
            return Err(format!(
                "the command line is {} bytes; the header holds 1535",
                c.len()
            ));
        }
        h.cmdline = Box::new([0u8; 1536]);
        h.cmdline[..c.len()].copy_from_slice(c.as_bytes());
    }
    let page = h.page_size as usize;
    let mut out = Vec::new();
    Header::V0(h)
        .write(&mut out)
        .map_err(|e| format!("cannot write the header: {e}"))?;
    for part in [&kernel[..], &ramdisk[..], img.second] {
        pad_to(&mut out, page);
        out.extend_from_slice(part);
    }
    pad_to(&mut out, page);
    let said = said(body, &img, &out)?;
    let out = match &footer {
        Some(_) if parts.no_footer => out,
        Some(f) => avb::add_footer(&out, control.len(), f)?,
        None => {
            // the dumped partition's size is kept: what trailed the image is zero here
            out.resize(out.len().max(control.len()), 0);
            out
        }
    };
    Ok((out, said))
}

/// What differs between the control (read) and the new image body, by region: the header page, the kernel,
/// the ramdisk ("differs: ..." lines; "differs: nothing" when equal). The footer is always rebuilt and the
/// result says so in its own line.
fn said(body: &[u8], control: &image::Image, new: &[u8]) -> Result<Vec<String>, String> {
    let n = image::read(new)?;
    let page = control.header.page_size as usize;
    let count = |a: &[u8], b: &[u8]| {
        if a.len() != b.len() {
            format!("{} bytes became {}", a.len(), b.len())
        } else {
            let d = a.iter().zip(b).filter(|(x, y)| x != y).count();
            format!("{d} of {} bytes", a.len())
        }
    };
    let mut lines = Vec::new();
    let old_page = &body[..page];
    if old_page != &new[..page] {
        lines.push(format!(
            "differs: header: {}",
            count(old_page, &new[..page])
        ));
    }
    if control.kernel != n.kernel {
        lines.push(format!(
            "differs: kernel: {}",
            count(control.kernel, n.kernel)
        ));
    }
    if control.ramdisk != n.ramdisk {
        lines.push(format!(
            "differs: ramdisk: {}",
            count(control.ramdisk, n.ramdisk)
        ));
    }
    if lines.is_empty() {
        lines.push("differs: nothing".into());
    }
    Ok(lines)
}
