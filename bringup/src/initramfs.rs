// SPDX-License-Identifier: MIT OR Apache-2.0
//! The bringup initramfs: a reproducible newc cpio made from a gen_init_cpio spec, the gadget address a
//! variable, compressed with the codec the device names. Replaces build-initramfs.sh.
//!
//! The archive is byte for byte what the kernel's `gen_init_cpio -t 0` makes (proof
//! bringup-cpio-matches-gen-init-cpio, tests/initramfs.rs): inode numbers from 721 in spec order, every
//! time 0, the trailer padded to 512. The spec lines are gen_init_cpio's own: dir, nod, file, slink.

use std::fs;
use std::io::Write;
use std::path::Path;

const S_IFDIR: u32 = 0o040000;
const S_IFCHR: u32 = 0o020000;
const S_IFBLK: u32 = 0o060000;
const S_IFREG: u32 = 0o100000;
const S_IFLNK: u32 = 0o120000;

/// The address every phone image answers on unless told another: 172.16.42.1, the ECM MAC byte 2a.
pub const DEFAULT_NET: u8 = 42;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Codec {
    Gzip,
    Xz,
}

impl Codec {
    pub fn parse(s: &str) -> Result<Codec, String> {
        match s {
            "gzip" | "gz" => Ok(Codec::Gzip),
            "xz" => Ok(Codec::Xz),
            other => Err(format!("codec {other}: gzip or xz")),
        }
    }
}

enum Entry {
    Dir { mode: u32 },
    Nod { mode: u32, major: u32, minor: u32 },
    File { mode: u32, data: Vec<u8> },
    Slink { mode: u32, target: String },
}

struct Item {
    uid: u32,
    gid: u32,
    name: String,
    entry: Entry,
}

fn num(s: &str, line: &str) -> Result<u32, String> {
    s.parse::<u32>()
        .map_err(|_| format!("`{line}`: {s} is not a decimal number"))
}

/// The spec read: files named relative to `root` are read now. `net` rewrites the gadget address in every
/// file's text (see [`rewrite_net`]), and a file that still holds the default address afterwards fails the build.
fn parse(spec: &str, root: &Path, net: u8) -> Result<Vec<Item>, String> {
    let mut items = Vec::new();
    for raw in spec.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let f: Vec<&str> = line.split_whitespace().collect();
        let need = |n: usize| {
            if f.len() != n {
                Err(format!(
                    "`{line}`: exactly {n} fields (hard links are not supported)"
                ))
            } else {
                Ok(())
            }
        };
        let (name, uid, gid, entry) = match f[0] {
            "dir" => {
                need(5)?;
                (
                    f[1],
                    num(f[3], line)?,
                    num(f[4], line)?,
                    Entry::Dir {
                        mode: S_IFDIR | oct(f[2], line)?,
                    },
                )
            }
            "nod" => {
                need(8)?;
                let kind = match f[5] {
                    "c" => S_IFCHR,
                    "b" => S_IFBLK,
                    k => return Err(format!("`{line}`: device kind {k} is c or b")),
                };
                (
                    f[1],
                    num(f[3], line)?,
                    num(f[4], line)?,
                    Entry::Nod {
                        mode: kind | oct(f[2], line)?,
                        major: num(f[6], line)?,
                        minor: num(f[7], line)?,
                    },
                )
            }
            "slink" => {
                need(6)?;
                (
                    f[1],
                    num(f[4], line)?,
                    num(f[5], line)?,
                    Entry::Slink {
                        mode: S_IFLNK | oct(f[3], line)?,
                        target: f[2].to_string(),
                    },
                )
            }
            "file" => {
                need(6)?;
                let path = root.join(f[2]);
                let mut data = fs::read(&path)
                    .map_err(|e| format!("`{line}`: cannot read {}: {e}", path.display()))?;
                if net != DEFAULT_NET {
                    data = rewrite_net(data, net, f[1])?;
                }
                (
                    f[1],
                    num(f[4], line)?,
                    num(f[5], line)?,
                    Entry::File {
                        mode: S_IFREG | oct(f[3], line)?,
                        data,
                    },
                )
            }
            other => return Err(format!("`{line}`: {other} is not dir, nod, file or slink")),
        };
        if name.len() >= 4096
            || matches!(&entry, Entry::Slink { target, .. } if target.len() >= 4096)
        {
            return Err(format!(
                "`{line}`: a name or target of 4096 bytes or more (PATH_MAX)"
            ));
        }
        if matches!(&entry, Entry::File { data, .. } if u32::try_from(data.len()).is_err()) {
            return Err(format!(
                "`{line}`: the file is over the 4 GiB a newc cpio holds"
            ));
        }
        items.push(Item {
            uid,
            gid,
            name: name.trim_start_matches('/').to_string(),
            entry,
        });
    }
    Ok(items)
}

/// The gadget address as a variable: `172.16.42.` becomes `172.16.NET.` and the ECM MAC byte `ac:10:2a:`
/// becomes `ac:10:NN:` (NN the net in hex), in a text file. A file that still holds `172.16.42` afterwards
/// fails: the old address is never left behind.
pub fn rewrite_net(data: Vec<u8>, net: u8, name: &str) -> Result<Vec<u8>, String> {
    let Ok(text) = std::str::from_utf8(&data) else {
        return Ok(data);
    };
    let out = text
        .replace("172.16.42.", &format!("172.16.{net}."))
        .replace("ac:10:2a:", &format!("ac:10:{net:02x}:"))
        .replace("AC:10:2A:", &format!("AC:10:{net:02X}:"));
    let lower = out.to_ascii_lowercase();
    if lower.contains("172.16.42") || lower.contains("ac:10:2a:") {
        return Err(format!(
            "{name}: the address rewrite left 172.16.42 or ac:10:2a behind"
        ));
    }
    Ok(out.into_bytes())
}

fn pad4(out: &mut Vec<u8>) {
    while !out.len().is_multiple_of(4) {
        out.push(0);
    }
}

#[allow(clippy::too_many_arguments)]
fn header(
    out: &mut Vec<u8>,
    ino: u32,
    mode: u32,
    uid: u32,
    gid: u32,
    nlink: u32,
    filesize: usize,
    rdev: (u32, u32),
    namesize: usize,
) {
    out.extend(
        format!(
            "070701{ino:08X}{mode:08X}{uid:08X}{gid:08X}{nlink:08X}{:08X}{filesize:08X}{:08X}{:08X}{:08X}{:08X}{namesize:08X}{:08X}",
            0, 3, 1, rdev.0, rdev.1, 0
        )
        .bytes(),
    );
}

/// The newc archive of the spec, as gen_init_cpio -t 0 writes it.
pub fn cpio(spec: &str, root: &Path, net: u8) -> Result<Vec<u8>, String> {
    let items = parse(spec, root, net)?;
    let mut out = Vec::new();
    for (ino, it) in (721u32..).zip(&items) {
        let namesize = it.name.len() + 1;
        let (mode, nlink, data, rdev, slink): (u32, u32, &[u8], (u32, u32), bool) = match &it.entry
        {
            Entry::Dir { mode } => (*mode, 2, &[], (0, 0), false),
            Entry::Nod { mode, major, minor } => (*mode, 1, &[], (*major, *minor), false),
            Entry::File { mode, data } => (*mode, 1, data, (0, 0), false),
            Entry::Slink { mode, target } => (*mode, 1, target.as_bytes(), (0, 0), true),
        };
        header(
            &mut out,
            ino,
            mode,
            it.uid,
            it.gid,
            nlink,
            data.len() + usize::from(slink),
            rdev,
            namesize,
        );
        out.extend(it.name.bytes());
        out.push(0);
        pad4(&mut out);
        out.extend(data);
        if slink {
            out.push(0);
        }
        pad4(&mut out);
    }
    // the trailer, then the archive padded to 512 as the kernel's tool does
    out.extend(
        format!(
            "070701{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}{:08X}",
            0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 11, 0
        )
        .bytes(),
    );
    out.extend(b"TRAILER!!!\0");
    while !out.len().is_multiple_of(512) {
        out.push(0);
    }
    Ok(out)
}

/// The archive compressed with fixed settings, so the same inputs give the same bytes: gzip level 9 with no
/// name and no time; xz preset 6 with a CRC32 check (the 4.4 kernel's xz decoder reads CRC32 or none).
pub fn compress(raw: &[u8], codec: Codec) -> Result<Vec<u8>, String> {
    match codec {
        Codec::Gzip => {
            let mut e = flate2::GzBuilder::new().write(Vec::new(), flate2::Compression::new(9));
            e.write_all(raw).map_err(|e| e.to_string())?;
            e.finish().map_err(|e| e.to_string())
        }
        Codec::Xz => {
            let stream = xz2::stream::Stream::new_easy_encoder(6, xz2::stream::Check::Crc32)
                .map_err(|e| e.to_string())?;
            let mut e = xz2::write::XzEncoder::new_stream(Vec::new(), stream);
            e.write_all(raw).map_err(|e| e.to_string())?;
            e.finish().map_err(|e| e.to_string())
        }
    }
}

/// The initramfs file: the spec read (its files beside it, or under `root`), the archive made, compressed.
pub fn build(spec: &Path, root: Option<&Path>, net: u8, codec: Codec) -> Result<Vec<u8>, String> {
    let text =
        fs::read_to_string(spec).map_err(|e| format!("cannot read {}: {e}", spec.display()))?;
    let beside = spec
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let raw = cpio(&text, root.unwrap_or(beside), net)?;
    compress(&raw, codec)
}

/// A mode is octal, as gen_init_cpio reads it (755, 1777).
fn oct(s: &str, line: &str) -> Result<u32, String> {
    u32::from_str_radix(s, 8).map_err(|_| format!("`{line}`: {s} is not an octal mode"))
}
