// SPDX-License-Identifier: MIT OR Apache-2.0
//! The boot-format interface (item osfact-boot-adapter): the declaration's [boot] table is read, the format it
//! names (android-bootimg) packs a kernel and ramdisk into an image, and the same format counts that image back
//! against the declaration. Inputs: tests/fixtures/google-walleye/device.toml (the declaration, copied from the
//! lab) and tests/fixtures/boot/ (the kernel, ramdisk and appended device tree lpoco-bringup's tests used).

use osfact::boot::{self, Boot, Verdict};
use std::fs;
use std::path::{Path, PathBuf};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn declaration() -> Boot {
    let text = fs::read_to_string(fixtures().join("google-walleye/device.toml")).unwrap();
    Boot::from_declaration(&text).expect("the declaration's [boot] is read")
}

fn kernel_with_dtb() -> Vec<u8> {
    let mut k = fs::read(fixtures().join("boot/kernel")).unwrap();
    k.extend(fs::read(fixtures().join("boot/appended.dtb")).unwrap());
    k
}

#[test]
fn the_declarations_boot_table_is_read() {
    let b = declaration();
    assert_eq!(
        (b.kernel_offset, b.ramdisk_offset, b.pagesize),
        (0x8000, 0x1000000, 4096)
    );
    assert_eq!(b.msm_id, Some(vec![0x124, 0x20001]));
}

#[test]
fn a_declaration_without_boot_is_refused_in_words() {
    assert_eq!(
        Boot::from_declaration("[device]\ncodename = \"x\"\n").unwrap_err(),
        "the declaration has no [boot] table"
    );
}

#[test]
fn the_named_format_packs_and_counts_its_own_image() {
    let b = declaration();
    let format = boot::format(&b.format).expect("the default format exists");
    let ramdisk = fs::read(fixtures().join("boot/ramdisk")).unwrap();
    let image = format.pack(&b, &kernel_with_dtb(), &ramdisk).unwrap();
    let findings = format.check(&image, &b);
    let bad: Vec<String> = findings
        .iter()
        .filter(|f| matches!(f.verdict, Verdict::Fail(_)))
        .map(|f| f.line())
        .collect();
    assert!(bad.is_empty(), "{bad:?}");
    assert!(findings
        .iter()
        .any(|f| f.name == "qcom,msm-id" && matches!(f.verdict, Verdict::Ok(_))));
}

#[test]
fn a_format_nobody_wrote_is_refused_naming_the_known() {
    let e = boot::format("lk-uimage").err().expect("refused");
    assert!(e.contains("android-bootimg"), "{e}");
}
