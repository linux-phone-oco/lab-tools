// SPDX-License-Identifier: MIT OR Apache-2.0
//! Boot images, header v0: packed byte for byte as mkbootimg writes them, read back, and counted against what
//! the device's declaration ([boot]) says they must be.
//!
//! The header layer is abootimg-oxide (parse and write). It writes v0 header bytes exactly as mkbootimg does,
//! but two things are ours: the header's id - mkbootimg's v0 id is
//! a SHA-1 over kernel, ramdisk and second, each followed by its size, where the crate's helper hashes five
//! parts - and the padding after the last part, which the crate's full_write only seeks to.

use crate::boot::{os_words, Boot, BootFormat, Finding, Verdict};
use crate::{avb, fdt};
use abootimg_oxide::{Header, HeaderV0, HeaderV0Versioned, OsVersionPatch};
use sha1::{Digest, Sha1};
use sha2::Digest as _;
use std::io::Cursor;

pub const MAGIC: &[u8; 8] = b"ANDROID!";
const CMDLINE_MAX: usize = 1535;

pub fn pad_to(out: &mut Vec<u8>, page: usize) {
    let r = out.len() % page;
    if r != 0 {
        out.resize(out.len() + page - r, 0);
    }
}

/// The image mkbootimg would write for these parts and these header facts (no second image).
pub fn pack(boot: &Boot, kernel: &[u8], ramdisk: &[u8]) -> Result<Vec<u8>, String> {
    if boot.header_version != 0 {
        return Err(format!(
            "header_version {} is not supported yet: this tool packs header v0",
            boot.header_version
        ));
    }
    if ![2048, 4096, 8192, 16384].contains(&boot.pagesize) {
        return Err(format!(
            "pagesize {} is not one of 2048, 4096, 8192, 16384",
            boot.pagesize
        ));
    }
    if boot.cmdline.len() > CMDLINE_MAX {
        return Err(format!(
            "the command line is {} bytes; the header holds {CMDLINE_MAX}",
            boot.cmdline.len()
        ));
    }
    let addr = |off: u32, what: &str| {
        boot.base
            .checked_add(off)
            .ok_or_else(|| format!("base + {what} does not fit 32 bits"))
    };
    let mut cmdline = Box::new([0u8; 1536]);
    cmdline[..boot.cmdline.len()].copy_from_slice(boot.cmdline.as_bytes());
    let id = v0_id(kernel, ramdisk, &[]);
    let header = HeaderV0 {
        kernel_size: kernel.len() as u32,
        kernel_addr: addr(boot.kernel_offset, "kernel_offset")?,
        ramdisk_size: ramdisk.len() as u32,
        ramdisk_addr: addr(boot.ramdisk_offset, "ramdisk_offset")?,
        // no second image: mkbootimg writes size 0 and address 0 whatever second_offset says
        second_bootloader_size: 0,
        second_bootloader_addr: 0,
        tags_addr: addr(boot.tags_offset, "tags_offset")?,
        page_size: boot.pagesize,
        osversionpatch: OsVersionPatch(boot.os_word()?),
        board_name: [0; 16],
        cmdline,
        hash_digest: id,
        versioned: HeaderV0Versioned::V0,
    };
    let page = boot.pagesize as usize;
    let mut out = Vec::new();
    Header::V0(header.clone())
        .write(&mut out)
        .map_err(|e| format!("cannot write the header: {e}"))?;
    pad_to(&mut out, page);
    out.extend_from_slice(kernel);
    pad_to(&mut out, page);
    out.extend_from_slice(ramdisk);
    pad_to(&mut out, page);
    debug_assert_eq!(out.len(), header.boot_image_size());
    if boot.avb_footer {
        return with_footer(out, boot);
    }
    Ok(out)
}

/// The image grown to the boot partition with the AVB hash footer (algorithm NONE, sha256 digest). The salt is the
/// image's own digest, so the same inputs give the same bytes (avbtool draws a random one).
fn with_footer(image: Vec<u8>, boot: &Boot) -> Result<Vec<u8>, String> {
    let partition = boot
        .partition_size
        .ok_or("avb_footer = true needs partition_size: the footer sits at the partition's end")?;
    let salt = sha2::Sha256::digest(&image).to_vec();
    let footer = avb::Footer {
        original: image.len(),
        partition: "boot".into(),
        salt,
        hash_algorithm: "sha256".into(),
        rollback_index: 0,
        do_not_use_ab: false,
        props: vec![],
        held: vec![],
        described: 0,
        release: "osfact".into(),
    };
    avb::add_footer(&image, partition as usize, &footer)
}

/// A boot image read back: its header and its parts.
pub struct Image<'a> {
    pub header: HeaderV0,
    pub kernel: &'a [u8],
    pub ramdisk: &'a [u8],
    pub second: &'a [u8],
    pub len: usize,
}

pub fn read(bytes: &[u8]) -> Result<Image<'_>, String> {
    if bytes.get(..8) != Some(&MAGIC[..]) {
        return Err("not a boot image: no ANDROID! magic".into());
    }
    let header = match Header::parse(&mut Cursor::new(bytes)).map_err(|e| format!("header: {e}"))? {
        Header::V0(h) if h.header_version() == 0 => h,
        Header::V0(h) => {
            return Err(format!(
                "header version {} is not supported yet: this tool reads header v0",
                h.header_version()
            ))
        }
        Header::V3(h) => {
            return Err(format!(
                "header version {} is not supported yet: this tool reads header v0",
                h.header_version()
            ))
        }
    };
    let part = |pos: usize, size: u32| {
        bytes.get(pos..pos + size as usize).ok_or_else(|| {
            format!(
                "the image is cut short: a part of {size} bytes at {pos} runs past its {} bytes",
                bytes.len()
            )
        })
    };
    Ok(Image {
        kernel: part(header.kernel_position(), header.kernel_size)?,
        ramdisk: part(header.ramdisk_position(), header.ramdisk_size)?,
        second: part(
            header.second_bootloader_position(),
            header.second_bootloader_size,
        )?,
        len: bytes.len(),
        header,
    })
}

fn cmdline_of(h: &HeaderV0) -> String {
    let end = h
        .cmdline
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(h.cmdline.len());
    String::from_utf8_lossy(&h.cmdline[..end]).into_owned()
}

impl Image<'_> {
    /// The header's facts as a `[boot]` table, as a declaration would say them (the header holds addresses, so
    /// they are shown as offsets from base 0).
    pub fn info(&self) -> String {
        let h = &self.header;
        let (v, p) = os_words(h.osversionpatch.0);
        let mut s = String::from("[boot]\nheader_version = 0\nbase = 0x0\n");
        s += &format!("kernel_offset = {:#x}\n", h.kernel_addr);
        s += &format!("ramdisk_offset = {:#x}\n", h.ramdisk_addr);
        if h.second_bootloader_size != 0 {
            s += &format!("second_offset = {:#x}\n", h.second_bootloader_addr);
        }
        s += &format!("tags_offset = {:#x}\n", h.tags_addr);
        s += &format!("pagesize = {}\n", h.page_size);
        if let Some(v) = v {
            s += &format!("os_version = \"{v}\"\n");
        }
        if let Some(p) = p {
            s += &format!("os_patch_level = \"{p}\"\n");
        }
        s += &format!("cmdline = {:?}\n", cmdline_of(h));
        s += &format!(
            "# kernel {} bytes, ramdisk {} bytes, second {} bytes; the image is {} bytes\n",
            self.kernel.len(),
            self.ramdisk.len(),
            self.second.len(),
            self.len
        );
        if let Some((at, total)) = fdt::appended(self.kernel) {
            s += &format!("# a device tree of {total} bytes is appended to the kernel at {at}\n");
            if let Some(id) = fdt::root_property(&self.kernel[at..], "qcom,msm-id") {
                let cells: Vec<String> = id.iter().map(|c| format!("{c:#x}")).collect();
                s += &format!("msm_id = [{}]\n", cells.join(", "));
            }
        }
        s
    }

    /// Where the appended device tree starts in the kernel blob, if one is.
    pub fn dtb(&self) -> Option<&[u8]> {
        fdt::appended(self.kernel).map(|(at, _)| &self.kernel[at..])
    }
}

fn eq<T: PartialEq + std::fmt::Debug>(
    name: &'static str,
    got: T,
    want: T,
    show: impl Fn(&T) -> String,
) -> Finding {
    let verdict = if got == want {
        Verdict::Ok(show(&got))
    } else {
        Verdict::Fail(format!("got {}, want {}", show(&got), show(&want)))
    };
    Finding { name, verdict }
}

fn hex(v: &u32) -> String {
    format!("{v:#x}")
}

/// What a boot image must be for the device whose declaration says `boot`: magic, header v0, the size within the
/// partition, the offsets, the page size, the os words, the command line, and the appended device tree with its
/// qcom,msm-id. Returns the findings; a magic that is wrong ends the list there.
pub fn check(bytes: &[u8], boot: &Boot) -> Vec<Finding> {
    let magic_ok = bytes.get(..8) == Some(&MAGIC[..]);
    let mut out = vec![Finding {
        name: "magic",
        verdict: if magic_ok {
            Verdict::Ok("ANDROID!".into())
        } else {
            Verdict::Fail("got no ANDROID! at the start".into())
        },
    }];
    let img = match read(bytes) {
        Ok(i) if magic_ok => i,
        Ok(_) => return out,
        Err(e) => {
            out.push(Finding {
                name: "header version",
                verdict: Verdict::Fail(e),
            });
            return out;
        }
    };
    let h = &img.header;
    out.push(eq("header version", h.header_version(), 0, |v| {
        v.to_string()
    }));
    out.push(match boot.partition_size {
        Some(max) => Finding {
            name: "size within partition",
            verdict: if img.len as u64 <= max {
                Verdict::Ok(format!("{} of {max} bytes", img.len))
            } else {
                Verdict::Fail(format!("got {} bytes, the partition holds {max}", img.len))
            },
        },
        None => Finding {
            name: "size within partition",
            verdict: Verdict::NotMeasured("[boot] has no partition_size".into()),
        },
    });
    let want = |off: u32| boot.base.wrapping_add(off);
    out.push(eq(
        "kernel offset",
        h.kernel_addr,
        want(boot.kernel_offset),
        hex,
    ));
    out.push(eq(
        "ramdisk offset",
        h.ramdisk_addr,
        want(boot.ramdisk_offset),
        hex,
    ));
    out.push(eq("tags offset", h.tags_addr, want(boot.tags_offset), hex));
    // an image without a second part carries 0 here (mkbootimg writes 0 whatever second_offset says)
    let second_want = if h.second_bootloader_size == 0 {
        0
    } else {
        want(boot.second_offset)
    };
    out.push(eq(
        "second address",
        h.second_bootloader_addr,
        second_want,
        hex,
    ));
    out.push(eq("page size", h.page_size, boot.pagesize, |v| {
        v.to_string()
    }));
    out.push(match boot.os_word() {
        Ok(w) => eq("os words", h.osversionpatch.0, w, |v| {
            let (a, b) = os_words(*v);
            format!(
                "{} {}",
                a.unwrap_or("none".into()),
                b.unwrap_or("none".into())
            )
        }),
        Err(e) => Finding {
            name: "os words",
            verdict: Verdict::Fail(format!("the declaration: {e}")),
        },
    });
    out.push(eq("cmdline", cmdline_of(h), boot.cmdline.clone(), |s| {
        format!("{s:?}")
    }));
    if boot.avb_footer {
        out.push(match avb::verify(bytes) {
            Ok(true) => Finding {
                name: "avb footer",
                verdict: Verdict::Ok("the hash descriptor matches the image".into()),
            },
            Ok(false) => Finding {
                name: "avb footer",
                verdict: Verdict::Fail("the hash descriptor does not match the image".into()),
            },
            Err(e) => Finding {
                name: "avb footer",
                verdict: Verdict::Fail(e),
            },
        });
    }
    match img.dtb() {
        Some(dtb) => {
            out.push(Finding {
                name: "appended dtb",
                verdict: Verdict::Ok(format!("{} bytes after the kernel", dtb.len())),
            });
            out.push(match &boot.msm_id {
                Some(want) => {
                    let got = fdt::root_property(dtb, "qcom,msm-id").unwrap_or_default();
                    eq("qcom,msm-id", got, want.clone(), |v| {
                        v.iter().map(hex).collect::<Vec<_>>().join(" ")
                    })
                }
                None => Finding {
                    name: "qcom,msm-id",
                    verdict: Verdict::NotMeasured("[boot] has no msm_id".into()),
                },
            });
        }
        None => out.push(Finding {
            name: "appended dtb",
            verdict: Verdict::Fail("no device tree ends the kernel blob".into()),
        }),
    }
    out
}

/// mkbootimg's v0 id: a SHA-1 over kernel, ramdisk and second, each followed by its size, in the header's
/// 32 bytes.
pub fn v0_id(kernel: &[u8], ramdisk: &[u8], second: &[u8]) -> [u8; 32] {
    let mut sha = Sha1::new();
    for part in [kernel, ramdisk, second] {
        sha.update(part);
        sha.update((part.len() as u32).to_le_bytes());
    }
    let mut id = [0u8; 32];
    id[..20].copy_from_slice(&sha.finalize());
    id
}

/// The android-bootimg format as a [`BootFormat`]: header v0 (see [`pack`], [`check`]).
pub struct AndroidBootimg;

impl AndroidBootimg {
    pub const NAME: &'static str = "android-bootimg";
}

impl BootFormat for AndroidBootimg {
    fn pack(&self, boot: &Boot, kernel: &[u8], ramdisk: &[u8]) -> Result<Vec<u8>, String> {
        pack(boot, kernel, ramdisk)
    }
    fn check(&self, image: &[u8], boot: &Boot) -> Vec<Finding> {
        check(image, boot)
    }
    fn cmdline_room(&self) -> usize {
        CMDLINE_MAX
    }
}
