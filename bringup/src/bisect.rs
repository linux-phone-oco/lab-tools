//! Bisect: the series of swaps a file declares, built from one control and counted. Each image is the control
//! with the parts its entry names replaced (see swap.rs), optionally with the kernel put in an lz4 frame as the
//! stock one is (lz4 is wrapped, like avbtool: independent 4 MiB blocks, content checksum, level 9) and
//! optionally without the AVB footer. After building, each image is counted against the control: exactly the
//! declared parts among ramdisk, kernel, dtb and command line differ, no other of the four, the footer verifies at
//! the control's size (or is absent where declared so). The second stage and other header fields are not compared.

use crate::fdt;
use crate::image;
use crate::swap::{self, Parts};
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Series {
    /// The control image the series swaps parts of (a path, relative to the series file).
    pub control: String,
    pub image: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub name: String,
    pub kernel: Option<String>,
    pub dtb: Option<String>,
    pub ramdisk: Option<String>,
    pub cmdline: Option<String>,
    /// Put the kernel in an lz4 frame before swapping it in.
    #[serde(default)]
    pub lz4: bool,
    /// Rebuild the AVB footer (true when not said).
    #[serde(default = "yes")]
    pub footer: bool,
}

fn yes() -> bool {
    true
}

/// What a series run said: a line per image and the counts.
pub struct Report {
    pub lines: Vec<String>,
    pub ok: usize,
    pub failed: usize,
}

pub fn read_series(path: &Path) -> Result<Series, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let s: Series = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    if s.image.is_empty() {
        return Err(format!("{}: no [[image]] declared", path.display()));
    }
    let mut seen = std::collections::BTreeSet::new();
    for e in &s.image {
        if e.name.is_empty() || e.name.contains(['/', '\\']) || e.name.starts_with('.') {
            return Err(format!(
                "{}: image name {:?} is not a plain name",
                path.display(),
                e.name
            ));
        }
        if !seen.insert(&e.name) {
            return Err(format!(
                "{}: image {} is declared twice",
                path.display(),
                e.name
            ));
        }
        if e.lz4 && e.kernel.is_none() {
            return Err(format!(
                "{}: image {}: lz4 without a kernel",
                path.display(),
                e.name
            ));
        }
    }
    Ok(s)
}

/// An lz4 frame of `data`, made by the lz4 program: level 9, independent 4 MiB blocks, content checksum.
pub fn lz4_frame(data: &[u8], tool: &str) -> Result<Vec<u8>, String> {
    let dir = swap::scratch("lz4")?;
    let src = dir.join("in");
    let result = fs::write(&src, data)
        .map_err(|e| format!("cannot write {}: {e}", src.display()))
        .and_then(|_| {
            let o = std::process::Command::new(tool)
                .args(["-9", "-BI", "-B7", "-q", "-c", src.to_str().unwrap()])
                .output()
                .map_err(|e| format!("cannot run {tool}: {e} (an lz4 kernel needs lz4)"))?;
            if o.status.success() {
                Ok(o.stdout)
            } else {
                Err(format!(
                    "{tool} failed: {}",
                    String::from_utf8_lossy(&o.stderr).trim()
                ))
            }
        });
    let _ = fs::remove_dir_all(&dir);
    result
}

fn split(blob: &[u8]) -> (&[u8], &[u8]) {
    match fdt::appended(blob) {
        Some((at, _)) => blob.split_at(at),
        None => (blob, &[]),
    }
}

fn cmdline(img: &image::Image) -> Vec<u8> {
    let c = &img.header.cmdline[..];
    c[..c.iter().position(|&b| b == 0).unwrap_or(c.len())].to_vec()
}

/// A footer verifies when avbtool accepts the image (it looks the partition up by file name: a copy named boot.img).
fn verifies(img: &[u8], tool: &str) -> Result<bool, String> {
    let dir = swap::scratch("verify")?;
    let p = dir.join("boot.img");
    let r = fs::write(&p, img)
        .map_err(|e| format!("cannot write {}: {e}", p.display()))
        .map(|_| {
            std::process::Command::new(tool)
                .args(["verify_image", "--image", p.to_str().unwrap()])
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        });
    let _ = fs::remove_dir_all(&dir);
    r
}

/// The image without its AVB footer and padding; an error when the footer claims more than the file holds.
fn body_len(img: &[u8]) -> Result<usize, String> {
    if !has_footer(img) {
        return Ok(img.len());
    }
    let n = u64::from_be_bytes(img[img.len() - 64 + 12..][..8].try_into().unwrap());
    if n > (img.len() - 64) as u64 {
        return Err("the AVB footer's original image size runs past the image".into());
    }
    Ok(n as usize)
}

fn has_footer(img: &[u8]) -> bool {
    img.len() >= 64 && &img[img.len() - 64..][..4] == b"AVBf"
}

/// Build every image of the series into `out` as boot-NAME.img and count each against the control.
pub fn run(series_path: &Path, out: &Path) -> Result<Report, String> {
    let series = read_series(series_path)?;
    let base = series_path.parent().unwrap_or(Path::new("."));
    let path = |p: &str| base.join(p);
    let rd =
        |p: &str| fs::read(path(p)).map_err(|e| format!("cannot read {}: {e}", path(p).display()));
    let control = rd(&series.control)?;
    let control_footer = has_footer(&control);
    let c = image::read(&control[..body_len(&control)?])?;
    let (c_kernel, c_dtb) = split(c.kernel);
    fs::create_dir_all(out).map_err(|e| format!("cannot make {}: {e}", out.display()))?;

    let mut report = Report {
        lines: Vec::new(),
        ok: 0,
        failed: 0,
    };
    for e in &series.image {
        let opt = |v: &Option<String>| v.as_deref().map(rd).transpose();
        let mut kernel = opt(&e.kernel)?;
        if e.lz4 {
            kernel = Some(lz4_frame(&kernel.unwrap(), "lz4")?);
        }
        let parts = Parts {
            kernel,
            dtb: opt(&e.dtb)?,
            ramdisk: opt(&e.ramdisk)?,
            cmdline: e.cmdline.clone(),
            no_footer: !e.footer,
        };
        let (img, _) = swap::swap(&control, &parts, "avbtool")?;
        let file = out.join(format!("boot-{}.img", e.name));
        fs::write(&file, &img).map_err(|e| format!("cannot write {}: {e}", file.display()))?;

        // the count: a part differs exactly when the entry declares it
        let n = image::read(&img[..body_len(&img)?])?;
        let (n_kernel, n_dtb) = split(n.kernel);
        let mut misses = Vec::new();
        let mut check = |what: &str, declared: bool, differs: bool| {
            if declared && !differs {
                misses.push(format!("{what} is declared but equals the control's"));
            } else if !declared && differs {
                misses.push(format!(
                    "{what} differs from the control's and is not declared"
                ));
            }
        };
        check("ramdisk", e.ramdisk.is_some(), n.ramdisk != c.ramdisk);
        check("kernel", e.kernel.is_some(), n_kernel != c_kernel);
        check("dtb", e.dtb.is_some(), n_dtb != c_dtb);
        check(
            "command line",
            e.cmdline.is_some(),
            cmdline(&n) != cmdline(&c),
        );
        if e.footer && control_footer {
            if !has_footer(&img) || img.len() != control.len() {
                misses.push(format!("no footer at the control's size {}", control.len()));
            } else if !verifies(&img, "avbtool")? {
                misses.push("the AVB footer does not verify".into());
            }
        } else if !e.footer && has_footer(&img) {
            misses.push("a footer is there though declared off".into());
        }
        let declared: Vec<&str> = [
            ("ramdisk", e.ramdisk.is_some()),
            ("kernel", e.kernel.is_some()),
            ("dtb", e.dtb.is_some()),
            ("command line", e.cmdline.is_some()),
        ]
        .iter()
        .filter(|(_, d)| *d)
        .map(|(n, _)| *n)
        .collect();
        let what = format!(
            "swaps {}{}{}",
            if declared.is_empty() {
                "nothing".to_string()
            } else {
                declared.join(", ")
            },
            if e.lz4 {
                " (kernel in an lz4 frame)"
            } else {
                ""
            },
            if e.footer { "" } else { "; no footer" },
        );
        if misses.is_empty() {
            report.ok += 1;
            report
                .lines
                .push(format!("ok   {} {what}, {} bytes", e.name, img.len()));
        } else {
            report.failed += 1;
            report
                .lines
                .push(format!("FAIL {} {what}: {}", e.name, misses.join("; ")));
        }
    }
    Ok(report)
}
