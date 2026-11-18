//! Image swap: a control image with one part replaced - ramdisk, kernel, device tree or command line - the
//! header's other facts kept and the AVB hash footer rebuilt over the new image (avbtool add_hash_footer,
//! algorithm NONE, the control's partition size, name, salt and properties; no Rust crate writes it, so it is
//! wrapped). The result says which regions differ from the control.

use crate::fdt;
use crate::image::{self, pad_to, v0_id};
use abootimg_oxide::Header;
use std::fs;
use std::process::Command;

/// The parts to put in; `None` keeps the control's.
#[derive(Default)]
pub struct Parts {
    pub kernel: Option<Vec<u8>>,
    pub dtb: Option<Vec<u8>>,
    pub ramdisk: Option<Vec<u8>>,
    pub cmdline: Option<String>,
}

const FOOTER: usize = 64;

/// The control's footer facts, as avbtool info_image says them.
struct Footer {
    original: usize,
    partition: String,
    salt: String,
    props: Vec<String>,
    hash_algorithm: String,
    rollback_index: String,
    do_not_use_ab: bool,
}

fn footer_of(control: &[u8], tool: &str) -> Result<Option<Footer>, String> {
    if control.len() < FOOTER || &control[control.len() - FOOTER..][..4] != b"AVBf" {
        return Ok(None);
    }
    let f = &control[control.len() - FOOTER..];
    let original = u64::from_be_bytes(f[12..20].try_into().unwrap()) as usize;
    if original > control.len() - FOOTER {
        return Err("the AVB footer's original image size runs past the image".into());
    }
    let dir = scratch("info")?;
    let path = dir.join("control.img");
    fs::write(&path, control).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let out = run(tool, &["info_image", "--image", path.to_str().unwrap()]);
    let _ = fs::remove_dir_all(&dir);
    let out = out?;
    let (mut partition, mut salt, mut props, mut algo) = (None, None, Vec::new(), None);
    let (mut hash, mut rollback, mut flags, mut descriptors) = (None, None, None, 0);
    let mut in_hash = false;
    for line in out.lines().map(str::trim) {
        if line.ends_with("descriptor:") {
            descriptors += 1;
            in_hash = line == "Hash descriptor:";
        } else if let Some(v) = line.strip_prefix("Hash Algorithm:") {
            hash = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Rollback Index Location:") {
            if v.trim() != "0" {
                return Err(
                    "the control's rollback index location is not 0: swap does not carry it".into(),
                );
            }
        } else if let Some(v) = line.strip_prefix("Rollback Index:") {
            rollback = Some(v.trim().to_string());
        } else if let (true, Some(v)) = (in_hash, line.strip_prefix("Flags:")) {
            flags = v.trim().parse::<u32>().ok();
        } else if let Some(v) = line.strip_prefix("Partition Name:") {
            partition = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Salt:") {
            salt = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Algorithm:") {
            algo = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("Prop:") {
            // Prop: key -> 'value'
            let (k, v) = v.split_once("->").ok_or("a Prop line without ->")?;
            let v = v.trim();
            let v = v
                .strip_prefix('\'')
                .and_then(|v| v.strip_suffix('\''))
                .unwrap_or(v);
            props.push(format!("{}:{v}", k.trim()));
        }
    }
    if algo.as_deref() != Some("NONE") {
        return Err(format!(
            "the control's vbmeta is signed ({:?}): swap rebuilds only algorithm NONE footers",
            algo
        ));
    }
    if descriptors != 1 || !in_hash {
        return Err(format!(
            "the control's vbmeta has {descriptors} descriptors: swap rebuilds only a footer with one hash descriptor"
        ));
    }
    let flags = flags.ok_or("the hash descriptor has no flags")?;
    if flags & !1 != 0 {
        return Err(format!(
            "the hash descriptor's flags {flags:#x} are not carried by swap"
        ));
    }
    Ok(Some(Footer {
        original,
        hash_algorithm: hash.ok_or("the footer has no hash algorithm")?,
        rollback_index: rollback.ok_or("the footer has no rollback index")?,
        do_not_use_ab: flags & 1 == 1,
        partition: partition.ok_or("the footer has no hash descriptor partition name")?,
        salt: salt.ok_or("the footer has no salt")?,
        props,
    }))
}

fn scratch(what: &str) -> Result<std::path::PathBuf, String> {
    let dir =
        std::env::temp_dir().join(format!("lpoco-bringup-swap-{}-{what}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).map_err(|e| format!("cannot make {}: {e}", dir.display()))?;
    Ok(dir)
}

fn run(tool: &str, args: &[&str]) -> Result<String, String> {
    let o = Command::new(tool)
        .args(args)
        .output()
        .map_err(|e| format!("cannot run {tool}: {e} (the AVB footer needs avbtool)"))?;
    if !o.status.success() {
        return Err(format!(
            "{tool} {} failed: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&o.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&o.stdout).into_owned())
}

/// The control with `parts` swapped in, and a line per region that differs from the control.
pub fn swap(
    control: &[u8],
    parts: &Parts,
    avbtool: &str,
) -> Result<(Vec<u8>, Vec<String>), String> {
    let footer = footer_of(control, avbtool)?;
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
        Some(f) => with_footer(out, control.len(), f, avbtool)?,
        None => {
            // the dumped partition's size is kept: what trailed the image is zero here
            out.resize(out.len().max(control.len()), 0);
            out
        }
    };
    Ok((out, said))
}

fn with_footer(
    mut image: Vec<u8>,
    partition_size: usize,
    f: &Footer,
    tool: &str,
) -> Result<Vec<u8>, String> {
    let dir = scratch("footer")?;
    let path = dir.join("new.img");
    let result = (|| {
        fs::write(&path, &image).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        let size = partition_size.to_string();
        let mut args = vec![
            "add_hash_footer",
            "--image",
            path.to_str().unwrap(),
            "--partition_size",
            &size,
            "--partition_name",
            &f.partition,
            "--algorithm",
            "NONE",
            "--salt",
            &f.salt,
        ];
        if f.do_not_use_ab {
            args.push("--do_not_use_ab");
        }
        args.extend([
            "--hash_algorithm",
            &f.hash_algorithm,
            "--rollback_index",
            &f.rollback_index,
        ]);
        for p in &f.props {
            args.extend(["--prop", p]);
        }
        run(tool, &args)?;
        image = fs::read(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Ok(image)
    })();
    let _ = fs::remove_dir_all(&dir);
    result
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
