// SPDX-License-Identifier: MIT OR Apache-2.0
//! image-avb-footer-in-rust, review findings: a footer that lies about its sizes is an error, never a panic.
//! The images are made here: a footer at the end pointing at a vbmeta blob whose header fields are set to
//! the values named (the layout is avbtool 1.4.0's: footer @12 original, @20 offset, @28 size; vbmeta @12
//! authentication size, @28 algorithm, @96/@104 descriptors, @128 release string).

use lpoco_bringup::avb;

/// An image of 4096 zero bytes, a vbmeta blob `meta`, and a footer over both.
fn image_with(meta: &[u8]) -> Vec<u8> {
    let mut img = vec![0u8; 4096];
    let offset = img.len() as u64;
    img.extend(meta);
    img.resize(8192 - 64, 0);
    img.extend(b"AVBf");
    img.extend(1u32.to_be_bytes());
    img.extend(0u32.to_be_bytes());
    img.extend(4096u64.to_be_bytes());
    img.extend(offset.to_be_bytes());
    img.extend((meta.len() as u64).to_be_bytes());
    img.resize(8192, 0);
    img
}

fn header(len: usize) -> Vec<u8> {
    let mut m = vec![0u8; len];
    m[..4].copy_from_slice(b"AVB0");
    m
}

#[test]
fn a_vbmeta_blob_shorter_than_its_header_is_refused() {
    let r = avb::read(&image_with(&header(128)));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn an_authentication_size_that_overflows_is_refused() {
    let mut m = header(256);
    m[12..20].copy_from_slice(&u64::MAX.to_be_bytes());
    let r = avb::read(&image_with(&m));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn a_hash_descriptor_shorter_than_its_fixed_part_is_refused() {
    let mut m = header(256);
    m[20..28].copy_from_slice(&80u64.to_be_bytes()); // aux block size
    m[104..112].copy_from_slice(&80u64.to_be_bytes()); // descriptors size
    let mut d = Vec::new();
    d.extend(2u64.to_be_bytes()); // tag: hash
    d.extend(64u64.to_be_bytes()); // 64 bytes follow: fewer than the 116 a hash descriptor has
    d.resize(16 + 64, 0);
    m.extend(d);
    let r = avb::read(&image_with(&m));
    assert!(r.is_err(), "{r:?}");
}

#[test]
fn a_footer_whose_descriptor_size_differs_from_the_images_does_not_verify() {
    let f = avb::Footer {
        original: 0,
        partition: "boot".into(),
        salt: vec![1, 2],
        hash_algorithm: "sha256".into(),
        rollback_index: 0,
        do_not_use_ab: false,
        props: vec![],
        held: vec![],
        described: 0,
        release: "avbtool 1.4.0".into(),
    };
    let body = vec![7u8; 5000];
    let mut img = avb::add_footer(&body, 131072, &f).unwrap();
    assert!(avb::verify(&img).unwrap(), "a footer made here verifies");
    let at = img.len() - 64;
    let offset = u64::from_be_bytes(img[at + 20..at + 28].try_into().unwrap()) as usize;
    // vbmeta header 256, descriptor header 16, image size first
    img[offset + 256 + 16..offset + 256 + 24].copy_from_slice(&4999u64.to_be_bytes());
    assert!(!avb::verify(&img).unwrap());
}
