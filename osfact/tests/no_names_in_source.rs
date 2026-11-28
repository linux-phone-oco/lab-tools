// SPDX-License-Identifier: MIT OR Apache-2.0
//! The seam check (decision 2026-11-27-osfact-own-rust-in-lab-tools; the author's word: generality designed in):
//! osfact's Rust source names a package format and a transport, never a distro or a device - those are data
//! (a distro profile, a device declaration). Fails if a name below appears anywhere in src/, comments included.

use std::fs;
use std::path::Path;

const NAMES: &[&str] = &[
    "alpine",
    "debian",
    "ubuntu",
    "fedora",
    "arch linux",
    "archlinux",
    "nixos",
    "postmarketos",
    "walleye",
    "surya",
    "pixel",
    "poco",
    "oneplus",
    "msm8998",
    "sm7150",
];

fn sources(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for e in fs::read_dir(dir).expect("src readable") {
        let p = e.expect("entry").path();
        if p.is_dir() {
            sources(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn no_distro_or_device_name_in_the_source_of_osfact() {
    let mut files = Vec::new();
    sources(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut files,
    );
    assert!(!files.is_empty(), "no source found");
    let mut hits = Vec::new();
    for f in files {
        for (n, line) in fs::read_to_string(&f).expect("read").lines().enumerate() {
            let low = line.to_lowercase();
            let words: Vec<&str> = low.split(|c: char| !c.is_alphanumeric()).collect();
            for name in NAMES.iter().filter(|name| words.contains(*name)) {
                hits.push(format!("{}:{}: {name}", f.display(), n + 1));
            }
        }
    }
    assert!(
        hits.is_empty(),
        "distro or device names in osfact's source: {hits:?}"
    );
}
