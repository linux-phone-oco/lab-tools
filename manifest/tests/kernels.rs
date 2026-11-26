// SPDX-License-Identifier: MIT OR Apache-2.0
//! `[kernels]`: the test kernels `lpoco lab kernel` keeps on the phone beside stock and the one kept as the
//! default, so a procedure reads their names from the device instead of carrying them.

use lpoco_manifest::Manifest;

fn with(kernels: &str) -> String {
    format!("[device]\ncodename = \"x\"\n[kernels]\n{kernels}\n")
}

#[test]
fn a_device_declares_its_test_kernels_and_the_one_kept() {
    let m = Manifest::parse(&with(
        "test = [\"calls4\", \"pmicwdt\"]\nkept = \"pmicwdt\"",
    ))
    .unwrap();
    let k = m.kernels.unwrap();
    assert_eq!(k.test, ["calls4", "pmicwdt"]);
    assert_eq!(k.kept.as_deref(), Some("pmicwdt"));
    assert!(Manifest::parse("[device]\ncodename = \"x\"\n")
        .unwrap()
        .kernels
        .is_none());
}

#[test]
fn a_kernel_name_devlab_would_refuse_is_refused_naming_it() {
    for bad in ["calls+", "Calls", "a b", "", "a_b"] {
        let e = Manifest::parse(&with(&format!("test = [{bad:?}]"))).unwrap_err();
        assert!(
            e.contains("kernels") && e.contains(&format!("{bad:?}")),
            "{bad}: {e}"
        );
    }
}

#[test]
fn a_kernel_listed_twice_or_kept_without_being_listed_is_refused() {
    let e = Manifest::parse(&with("test = [\"a\", \"a\"]")).unwrap_err();
    assert!(e.contains("\"a\"") && e.contains("twice"), "{e}");
    let e = Manifest::parse(&with("test = [\"a\"]\nkept = \"b\"")).unwrap_err();
    assert!(e.contains("kept") && e.contains("\"b\""), "{e}");
}

#[test]
fn a_kept_name_that_is_no_kernel_name_and_a_table_with_no_kernels_are_judged() {
    for bad in ["Bad", ""] {
        let e = Manifest::parse(&with(&format!("test = [\"a\"]\nkept = {bad:?}"))).unwrap_err();
        assert!(e.contains(&format!("{bad:?}")), "{bad}: {e}");
    }
    let k = Manifest::parse(&with("")).unwrap().kernels.unwrap();
    assert!(k.test.is_empty() && k.kept.is_none());
}

/// A device that boots its kernels from memory, with everything such a boot reads declared.
const FASTBOOT_DEVICE: &str = "[device]\ncodename = \"x\"\n[fastboot]\nserial = \"FA81\"\n\
[[host.gadget]]\nserial = \"s\"\naccess = [\"tty\"]\nusb_id = \"1d6b:0104\"\n\
[kernels]\ntest = [\"z21\"]\nboot = \"fastboot\"\nway_back = [\"true\"]\n[images.z21]\npath = \"images/z21.img\"\n";

#[test]
fn a_device_says_its_kernels_boot_from_memory_by_fastboot_and_entry_is_the_default() {
    let k = Manifest::parse(FASTBOOT_DEVICE).unwrap().kernels.unwrap();
    assert_eq!(k.boot, lpoco_manifest::KernelBoot::Fastboot);
    let k = Manifest::parse(&with("test = [\"a\"]"))
        .unwrap()
        .kernels
        .unwrap();
    assert_eq!(k.boot, lpoco_manifest::KernelBoot::Entry);
    let e = Manifest::parse(&with("boot = \"magic\"")).unwrap_err();
    assert!(e.contains("magic"), "{e}");
}

#[test]
fn a_fastboot_boot_that_lacks_what_it_reads_is_refused_naming_the_missing_piece() {
    let cut = |from: &str, to: &str| FASTBOOT_DEVICE.replace(from, to);
    let cases = [
        // (broken manifest, what the refusal names)
        (
            cut("[images.z21]\npath = \"images/z21.img\"\n", ""),
            "images.z21",
        ),
        (
            cut("[fastboot]\nserial = \"FA81\"\n", ""),
            "[fastboot] serial",
        ),
        (cut("usb_id = \"1d6b:0104\"\n", ""), "usb_id"),
    ];
    for (text, names) in cases {
        let e = Manifest::parse(&text).unwrap_err();
        assert!(e.contains("kernels") && e.contains(names), "{names}: {e}");
    }
}
