// SPDX-License-Identifier: MIT OR Apache-2.0
//! The AVB hash footer, read and written here (libavb's format as avbtool 1.4.0 writes it, algorithm NONE: a
//! vbmeta header, one hash descriptor and properties, no key and no signature). A footer sits in the last
//! block of the partition and points back at the vbmeta blob that follows the image; swap and bisect carry the
//! control's facts onto a new image with it, so no avbtool (Python) is run.
//! Sources: avbtool 1.4.0 (AvbFooter, AvbVBMetaHeader, AvbHashDescriptor, AvbPropertyDescriptor,
//! add_hash_footer); the proof is tests/avb_native.rs, which holds each byte to avbtool's.

use sha2::{Digest, Sha256, Sha512};

/// The footer's size and the block it sits in (avbtool's image block size).
pub const FOOTER: usize = 64;
const BLOCK: usize = 4096;
const VBMETA_HEADER: usize = 256;
const TAG_PROPERTY: u64 = 0;
const TAG_HASH: u64 = 2;
const HASH_DESCRIPTOR: usize = 132;

/// The facts a footer carries over to a new image.
#[derive(Clone, Debug, PartialEq)]
pub struct Footer {
    /// The image's size before the footer: what the digest covers.
    pub original: usize,
    pub partition: String,
    pub salt: Vec<u8>,
    pub hash_algorithm: String,
    pub rollback_index: u64,
    pub do_not_use_ab: bool,
    pub props: Vec<(String, Vec<u8>)>,
    /// The digest the descriptor holds (read; ignored when a footer is made).
    pub held: Vec<u8>,
    /// The image size the descriptor states (read; ignored when a footer is made).
    pub described: usize,
    /// avbtool's own words in the header ("avbtool 1.4.0"), kept so the bytes equal the control's.
    pub release: String,
}

fn be32(b: &[u8], at: usize) -> Result<u32, String> {
    b.get(at..at + 4)
        .map(|x| u32::from_be_bytes(x.try_into().unwrap()))
        .ok_or_else(|| "the AVB metadata is cut short".to_string())
}

fn be64(b: &[u8], at: usize) -> Result<u64, String> {
    b.get(at..at + 8)
        .map(|x| u64::from_be_bytes(x.try_into().unwrap()))
        .ok_or_else(|| "the AVB metadata is cut short".to_string())
}

fn slice(b: &[u8], at: usize, len: usize) -> Result<&[u8], String> {
    at.checked_add(len)
        .and_then(|end| b.get(at..end))
        .ok_or_else(|| "the AVB metadata is cut short".to_string())
}

fn round(n: usize, to: usize) -> usize {
    n.div_ceil(to) * to
}

/// The digest of salt then data, as the descriptor holds it.
pub fn digest(algorithm: &str, salt: &[u8], data: &[u8]) -> Result<Vec<u8>, String> {
    fn go<D: Digest>(salt: &[u8], data: &[u8]) -> Vec<u8> {
        let mut d = D::new();
        d.update(salt);
        d.update(data);
        d.finalize().to_vec()
    }
    match algorithm {
        "sha256" => Ok(go::<Sha256>(salt, data)),
        "sha512" => Ok(go::<Sha512>(salt, data)),
        other => Err(format!(
            "the hash algorithm {other} is not one swap writes (sha256, sha512)"
        )),
    }
}

/// The footer an image ends in, `None` when it ends in none. The image's digest is not checked (see [`verify`]).
pub fn read(image: &[u8]) -> Result<Option<Footer>, String> {
    if image.len() < FOOTER || &image[image.len() - FOOTER..][..4] != b"AVBf" {
        return Ok(None);
    }
    let f = &image[image.len() - FOOTER..];
    let original = be64(f, 12)? as usize;
    let (offset, size) = (be64(f, 20)? as usize, be64(f, 28)? as usize);
    if original > image.len() - FOOTER {
        return Err("the AVB footer's original image size runs past the image".into());
    }
    let meta = slice(image, offset, size)
        .map_err(|_| "the AVB footer's vbmeta blob runs past the image".to_string())?;
    if slice(meta, 0, 4)? != b"AVB0" {
        return Err("the footer points at no vbmeta header".into());
    }
    let (auth, aux) = (be64(meta, 12)? as usize, be64(meta, 20)? as usize);
    let algorithm = be32(meta, 28)?;
    if algorithm != 0 {
        return Err(format!(
            "the control's vbmeta is signed (algorithm type {algorithm}): swap rebuilds only algorithm NONE footers"
        ));
    }
    let location = be32(meta, 124)?;
    if location != 0 {
        return Err(
            "the control's rollback index location is not 0: swap does not carry it".into(),
        );
    }
    if be32(meta, 120)? != 0 {
        return Err("the control's vbmeta flags are not 0: swap does not carry them".into());
    }
    if be64(meta, 72)? != 0 {
        return Err(
            "the control's vbmeta has a public key: swap rebuilds only algorithm NONE footers"
                .into(),
        );
    }
    let rollback_index = be64(meta, 112)?;
    let release = slice(meta, 128, 48)?;
    let release =
        String::from_utf8_lossy(&release[..release.iter().position(|&b| b == 0).unwrap_or(48)])
            .into_owned();
    if auth != 0 {
        return Err("the control's vbmeta has an authentication block: swap rebuilds only algorithm NONE footers".into());
    }
    let aux_block = slice(meta, VBMETA_HEADER, aux)?;
    let (d_off, d_len) = (be64(meta, 96)? as usize, be64(meta, 104)? as usize);
    let mut d = slice(aux_block, d_off, d_len)?;

    let (mut hash, mut props) = (None, Vec::new());
    while !d.is_empty() {
        let (tag, following) = (be64(d, 0)?, be64(d, 8)? as usize);
        let desc = slice(d, 16, following)?;
        match tag {
            TAG_HASH if hash.is_none() => hash = Some(hash_descriptor(desc)?),
            TAG_HASH => return Err("the control's vbmeta has two hash descriptors: swap rebuilds only a footer with one".into()),
            TAG_PROPERTY => {
                let (k, v) = (be64(desc, 0)? as usize, be64(desc, 8)? as usize);
                let key = String::from_utf8(slice(desc, 16, k)?.to_vec())
                    .map_err(|_| "a property key is not UTF-8".to_string())?;
                props.push((key, slice(desc, 16 + k + 1, v)?.to_vec()));
            }
            other => {
                return Err(format!(
                    "the control's vbmeta has a descriptor of kind {other}: swap rebuilds only a footer with one hash descriptor and properties"
                ))
            }
        }
        d = &d[16 + following..];
    }
    let h = hash.ok_or("the control's vbmeta has no hash descriptor")?;
    if h.flags & !1 != 0 {
        return Err(format!(
            "the hash descriptor's flags {:#x} are not carried by swap",
            h.flags
        ));
    }
    Ok(Some(Footer {
        original,
        partition: h.partition,
        salt: h.salt,
        hash_algorithm: h.algorithm,
        rollback_index,
        do_not_use_ab: h.flags & 1 == 1,
        props,
        held: h.held,
        described: h.sized,
        release,
    }))
}

/// A hash descriptor's facts.
struct Hash {
    partition: String,
    salt: Vec<u8>,
    algorithm: String,
    flags: u32,
    held: Vec<u8>,
    sized: usize,
}

/// The hash descriptor's body (after its tag and length).
fn hash_descriptor(d: &[u8]) -> Result<Hash, String> {
    let name = &slice(d, 8, 32)?;
    let algorithm =
        String::from_utf8_lossy(&name[..name.iter().position(|&b| b == 0).unwrap_or(32)])
            .into_owned();
    let (n, s) = (be32(d, 40)? as usize, be32(d, 44)? as usize);
    let (flags, l) = (be32(d, 52)?, be32(d, 48)? as usize);
    let rest = d
        .get(HASH_DESCRIPTOR - 16..)
        .ok_or("the hash descriptor is cut short")?;
    let partition = String::from_utf8(slice(rest, 0, n)?.to_vec())
        .map_err(|_| "the partition name is not UTF-8".to_string())?;
    Ok(Hash {
        partition,
        salt: slice(rest, n, s)?.to_vec(),
        algorithm,
        flags,
        held: slice(rest, n + s, l)?.to_vec(),
        sized: be64(d, 0)? as usize,
    })
}

/// The footer verifies when its digest, over the original image with its salt, is the descriptor's.
pub fn verify(image: &[u8]) -> Result<bool, String> {
    let Some(f) = read(image)? else {
        return Ok(false);
    };
    Ok(f.described == f.original
        && f.held == digest(&f.hash_algorithm, &f.salt, &image[..f.original])?)
}

fn descriptor(tag: u64, body: Vec<u8>) -> Vec<u8> {
    let padded = round(body.len(), 8);
    let mut d = Vec::with_capacity(16 + padded);
    d.extend(tag.to_be_bytes());
    d.extend((padded as u64).to_be_bytes());
    d.extend(body);
    d.resize(16 + padded, 0);
    d
}

/// The vbmeta blob for `image` (what the digest covers) with the footer's facts.
fn vbmeta(image: &[u8], f: &Footer) -> Result<Vec<u8>, String> {
    let digest = digest(&f.hash_algorithm, &f.salt, image)?;
    if f.hash_algorithm.len() > 31 {
        return Err("the hash algorithm's name is too long".into());
    }
    let mut hash = Vec::new();
    hash.extend((image.len() as u64).to_be_bytes());
    let mut name = [0u8; 32];
    name[..f.hash_algorithm.len()].copy_from_slice(f.hash_algorithm.as_bytes());
    hash.extend(name);
    for n in [f.partition.len(), f.salt.len(), digest.len()] {
        hash.extend((n as u32).to_be_bytes());
    }
    hash.extend(u32::from(f.do_not_use_ab).to_be_bytes());
    hash.extend([0u8; 60]);
    hash.extend(f.partition.as_bytes());
    hash.extend(&f.salt);
    hash.extend(&digest);
    let mut descriptors = descriptor(TAG_HASH, hash);
    for (key, value) in &f.props {
        let mut p = Vec::new();
        p.extend((key.len() as u64).to_be_bytes());
        p.extend((value.len() as u64).to_be_bytes());
        p.extend(key.as_bytes());
        p.push(0);
        p.extend(value);
        p.push(0);
        descriptors.extend(descriptor(TAG_PROPERTY, p));
    }
    if f.release.len() > 47 {
        return Err("the release string is longer than 47 bytes".into());
    }
    let aux = round(descriptors.len(), 64);
    let mut h = Vec::with_capacity(VBMETA_HEADER + aux);
    h.extend(b"AVB0");
    h.extend(1u32.to_be_bytes());
    h.extend(u32::from(f.do_not_use_ab).to_be_bytes()); // libavb 1.1 for do_not_use_ab
    h.extend(0u64.to_be_bytes()); // authentication block: empty for NONE
    h.extend((aux as u64).to_be_bytes());
    h.extend(0u32.to_be_bytes()); // algorithm NONE
    for (offset, size) in [
        (0, 0),                 // hash
        (0, 0),                 // signature
        (descriptors.len(), 0), // public key
        (descriptors.len(), 0), // public key metadata
        (0, descriptors.len()), // descriptors
    ] {
        h.extend((offset as u64).to_be_bytes());
        h.extend((size as u64).to_be_bytes());
    }
    h.extend(f.rollback_index.to_be_bytes());
    h.extend(0u32.to_be_bytes()); // flags
    h.extend(0u32.to_be_bytes()); // rollback index location
    let mut release = [0u8; 48];
    release[..f.release.len()].copy_from_slice(f.release.as_bytes());
    h.extend(release);
    h.resize(VBMETA_HEADER, 0);
    h.extend(descriptors);
    h.resize(VBMETA_HEADER + aux, 0);
    Ok(h)
}

/// `image` with the footer made over it, as avbtool add_hash_footer lays it out in a partition of
/// `partition_size`: the image grown to a block, the vbmeta blob in whole blocks, zeros, and the footer in
/// the last block.
pub fn add_footer(image: &[u8], partition_size: usize, f: &Footer) -> Result<Vec<u8>, String> {
    if !partition_size.is_multiple_of(BLOCK) {
        return Err(format!(
            "the partition size {partition_size} is not a multiple of the block size {BLOCK}"
        ));
    }
    let blob = vbmeta(image, f)?;
    let offset = round(image.len(), BLOCK);
    let end = offset + round(blob.len(), BLOCK);
    if end + BLOCK > partition_size || blob.len() > 64 * 1024 {
        return Err(format!(
            "the image ({} bytes) and its AVB metadata do not fit the partition's {partition_size} bytes",
            image.len()
        ));
    }
    let mut out = image.to_vec();
    out.resize(offset, 0);
    out.extend(&blob);
    out.resize(partition_size - FOOTER, 0);
    out.extend(b"AVBf");
    out.extend(1u32.to_be_bytes());
    out.extend(0u32.to_be_bytes());
    out.extend((image.len() as u64).to_be_bytes());
    out.extend((offset as u64).to_be_bytes());
    out.extend((blob.len() as u64).to_be_bytes());
    out.resize(partition_size, 0);
    Ok(out)
}
