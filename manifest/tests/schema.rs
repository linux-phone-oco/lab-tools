// SPDX-License-Identifier: MIT OR Apache-2.0
//! device.toml declared once: a misspelling is refused, the committed schema is the projection, the lab's
//! manifests validate.

use lpoco_manifest::{schema, Manifest};

const HEAD: &str = "[device]\ncodename = \"p\"\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n";

#[test]
fn a_misspelled_table_is_refused() {
    let e = Manifest::parse(&format!(
        "{HEAD}[[flash.overlays]]\nfrom = \"a\"\nto = \"root:/b\"\n"
    ))
    .unwrap_err();
    assert!(e.contains("overlays"), "the refusal names the word: {e}");
}

#[test]
fn a_misspelled_field_and_a_misspelled_top_table_are_refused() {
    assert!(Manifest::parse(&format!("{HEAD}[flash]\nsector_sise = 512\n")).is_err());
    assert!(Manifest::parse(&format!("{HEAD}[hosts]\nwatch = true\n")).is_err());
    assert!(Manifest::parse(&format!("{HEAD}[tools]\nlpoko = \"x\"\n")).is_err());
}

#[test]
fn schema_is_current() {
    let committed =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/device.schema.json"))
            .expect("device.schema.json is committed beside the crate");
    assert_eq!(
        committed,
        schema(),
        "the committed schema is not the projection: regenerate with `cargo run -p lpoco-manifest -- schema > manifest/device.schema.json`"
    );
}

/// The device manifests of the lab, named by LPOCO_LAB_MANIFESTS (paths, space separated); the fixtures
/// beside the crate always run.
#[test]
fn every_lab_manifest_validates() {
    let fixtures = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
    let mut files: Vec<std::path::PathBuf> = std::fs::read_dir(fixtures)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    if let Ok(more) = std::env::var("LPOCO_LAB_MANIFESTS") {
        files.extend(more.split_whitespace().map(Into::into));
    }
    assert!(files.len() >= 3, "surya, walleye and template fixtures");
    for f in files {
        let text = std::fs::read_to_string(&f).unwrap();
        if let Err(e) = Manifest::parse(&text) {
            panic!("{}: {e}", f.display());
        }
    }
}

#[test]
fn a_gadget_word_that_would_break_the_inits_quotes_is_refused_when_the_manifest_is_read() {
    let boot = "[boot]\nkernel_offset = 0x8000\nramdisk_offset = 0x1000000\ntags_offset = 0x100\npagesize = 4096\ncmdline = \"x\"\n";
    for field in ["serial", "manufacturer", "product", "label"] {
        for bad in ["a\\\"b", "a$b", "a`b", "a@b", "a\\\\b", "a\\nb", ""] {
            let mut gadget = String::from("[boot.gadget]\n");
            for k in ["serial", "product"] {
                if k != field {
                    gadget.push_str(&format!("{k} = \"ok\"\n"));
                }
            }
            gadget.push_str(&format!("{field} = \"{bad}\"\n"));
            let e = Manifest::parse(&format!("{HEAD}{boot}{gadget}")).unwrap_err();
            assert!(
                e.contains(field),
                "the refusal names {field} for {bad:?}: {e}"
            );
        }
    }
    assert!(Manifest::parse(&format!(
        "{HEAD}{boot}[boot.gadget]\nserial = \"pixel-1\"\nproduct = \"Pixel 2 (mainline)\"\n"
    ))
    .is_ok());
}

#[test]
fn a_shell_word_cannot_be_made_from_a_word_that_would_break_the_quotes() {
    use lpoco_manifest::ShellWord;
    for bad in ["a\"b", "a$b", "a`b", "a@b", "a\\b", "a\nb", ""] {
        assert!(ShellWord::new(bad).is_err(), "{bad:?} is refused");
    }
    assert_eq!(ShellWord::new("pixel-1").unwrap().as_str(), "pixel-1");
}

const LAYER: &str = r#"
[layer]
name = "xiaomi-surya"
needs = ["qrtr", "tqftpserv>=1.2"]

[layer.kernel]
tree = "https://example.org/linux"
tag = "sm7150-v7.1-rc3"
config = "kernel/surya.config"
dtbs = ["qcom/sm7150-xiaomi-surya-huaxing.dtb"]

[layer.boot]
method = "efi-kernel-install"

[layer.firmware]
method = "extract-from-partitions"

[layer.initramfs]
modules = ["ufs_qcom"]

[[layer.file]]
from = "ucm/HiFi.conf"
to = "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf"
kind = "data"
"#;

#[test]
fn a_layer_table_parses_with_files_kernel_boot_firmware_and_modules() {
    let m = Manifest::parse(&format!("{HEAD}{LAYER}")).unwrap();
    let l = m.layer.expect("the layer table is read");
    assert_eq!(l.name, "xiaomi-surya");
    assert_eq!(l.needs, ["qrtr", "tqftpserv>=1.2"]);
    assert_eq!(l.kernel.unwrap().dtbs.len(), 1);
    assert_eq!(l.file.len(), 1);
    assert_eq!(l.initramfs.unwrap().modules, ["ufs_qcom"]);
}

#[test]
fn an_unknown_layer_field_is_refused_in_every_layer_table() {
    for (from, to) in [
        (
            "name = \"xiaomi-surya\"",
            "name = \"xiaomi-surya\"\nneed = []",
        ),
        ("tag = \"sm7150-v7.1-rc3\"", "tag = \"x\"\ntagg = \"y\""),
        (
            "method = \"efi-kernel-install\"",
            "method = \"efi-kernel-install\"\nmethods = 1",
        ),
        ("kind = \"data\"", "kind = \"data\"\nmode = \"0644\""),
    ] {
        let text = format!("{HEAD}{}", LAYER.replace(from, to));
        assert!(Manifest::parse(&text).is_err(), "{to} is refused");
    }
}

#[test]
fn a_layer_file_goes_under_usr_with_a_known_kind_and_boot_method() {
    for (from, to) in [
        ("/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf", "/etc/x.conf"),
        (
            "/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf",
            "/usr/../etc/x.conf",
        ),
        ("kind = \"data\"", "kind = \"bogus\""),
        ("efi-kernel-install", "bogus"),
        ("extract-from-partitions", "bogus"),
    ] {
        assert!(
            Manifest::parse(&format!("{HEAD}{}", LAYER.replace(from, to))).is_err(),
            "{to} is refused"
        );
    }
}

#[test]
fn a_layer_name_path_token_or_missing_boot_table_that_would_break_a_package_is_refused() {
    for (from, to) in [
        ("name = \"xiaomi-surya\"", "name = \"a b/../c\""),
        ("\"qrtr\"", "\"\""),
        ("\"ufs_qcom\"", "\"\""),
        ("from = \"ucm/HiFi.conf\"", "from = \"/etc/shadow\""),
        ("from = \"ucm/HiFi.conf\"", "from = \"../a\""),
        ("/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf", "/usr/"),
        ("/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf", "/usr//x/./y"),
        ("efi-kernel-install", "android-bootimg"),
    ] {
        assert!(
            Manifest::parse(&format!("{HEAD}{}", LAYER.replace(from, to))).is_err(),
            "{to} is refused"
        );
    }
    let twice = format!("{LAYER}[[layer.file]]\nfrom = \"b\"\nto = \"/usr/share/alsa/ucm2/Xiaomi/surya/HiFi.conf\"\nkind = \"data\"\n");
    assert!(Manifest::parse(&format!("{HEAD}{twice}")).is_err());
}

#[test]
fn an_unknown_field_is_refused_in_the_firmware_and_initramfs_tables() {
    for (from, to) in [
        (
            "method = \"extract-from-partitions\"",
            "method = \"extract-from-partitions\"\nx = 1",
        ),
        ("modules = [\"ufs_qcom\"]", "modules = []\nmodule = 1"),
    ] {
        assert!(Manifest::parse(&format!("{HEAD}{}", LAYER.replace(from, to))).is_err());
    }
}
