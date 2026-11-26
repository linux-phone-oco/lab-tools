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

#[test]
fn an_images_kernel_tag_is_an_int_tag() {
    let ok = |t: &str| {
        Manifest::parse(&format!(
            "{HEAD}[images.a]\npath = \"a.img\"\nkernel_tag = \"{t}\"\n"
        ))
    };
    assert!(ok("int/surya/ufs-off/v7.1/1").is_ok());
    for bad in ["v7.1", "int/", "int/a b"] {
        assert!(ok(bad).is_err(), "{bad} is not an int/ tag");
    }
}

#[test]
fn a_recipe_names_another_image_and_files_by_sha256_and_needs_its_own_sha256() {
    let sha = "ab".repeat(32);
    let images = |recipe: &str, own: &str| {
        Manifest::parse(&format!(
            "{HEAD}[images.a]\npath = \"a.img\"\nsha256 = \"{sha}\"\n\n[images.b]\npath = \"b.img\"\n{own}\n[images.b.recipe]\n{recipe}\n"
        ))
    };
    let own = format!("sha256 = \"{sha}\"");
    assert!(images(
        &format!("control = \"a\"\ndtb = \"{sha}\"\ncmdline = \"console=ttyMSM0\""),
        &own
    )
    .is_ok());
    for (what, recipe, own) in [
        (
            "an image that does not declare its sha256",
            "control = \"a\"".to_string(),
            String::new(),
        ),
        (
            "a control that is not an image",
            "control = \"zz\"".to_string(),
            own.clone(),
        ),
        (
            "itself as control",
            "control = \"b\"".to_string(),
            own.clone(),
        ),
        (
            "a part that is not a sha256",
            "control = \"a\"\ndtb = \"z21.dtb\"".to_string(),
            own.clone(),
        ),
        (
            "a path where a sha256 goes",
            "control = \"a\"\nramdisk = \"../r\"".to_string(),
            own.clone(),
        ),
        (
            "a misspelt key",
            "control = \"a\"\ndtbs = \"x\"".to_string(),
            own.clone(),
        ),
    ] {
        assert!(images(&recipe, &own).is_err(), "{what} was accepted");
    }
}

#[test]
fn recipes_that_lead_back_to_themselves_are_refused_naming_the_chain() {
    let sha = "ab".repeat(32);
    let two = format!(
        "{HEAD}[images.a]\npath = \"a.img\"\nsha256 = \"{sha}\"\n[images.a.recipe]\ncontrol = \"b\"\n\n\
         [images.b]\npath = \"b.img\"\nsha256 = \"{sha}\"\n[images.b.recipe]\ncontrol = \"a\"\n"
    );
    let e = Manifest::parse(&two).expect_err("a cycle was accepted");
    assert!(
        e.contains("a") && e.contains("b") && e.contains("cycle"),
        "{e}"
    );
}

#[test]
fn a_recipes_command_line_must_fit_the_header() {
    let sha = "ab".repeat(32);
    let with = |n: usize| {
        Manifest::parse(&format!(
            "{HEAD}[images.a]\npath = \"a.img\"\nsha256 = \"{sha}\"\n\n[images.b]\npath = \"b.img\"\nsha256 = \"{sha}\"\n[images.b.recipe]\ncontrol = \"a\"\ncmdline = \"{}\"\n",
            "x".repeat(n)
        ))
    };
    assert!(with(1535).is_ok());
    assert!(
        with(1536).is_err(),
        "the header holds 1535 bytes (bringup swap)"
    );
}

/// `[kernel_builds.NAME]` (item image-recipe-kernel-from-int-tag): a kernel file made from an int/ tag in the
/// sandbox. Every value Nix needs to reproduce it is declared: the tag and its commit, the config and the tree
/// edits by sha256, the time the build's clock is fixed to, and the sha256 the result must have (research
/// image-recipes-how-it-stands, measurement 4: the time of day and the environment were what made hand builds differ).
#[test]
fn a_kernel_build_names_its_tag_and_commit_and_every_input_by_sha256() {
    let sha = "ab".repeat(32);
    let rev = "0e20c7a89ac5748fbd38f59a2b66324aad84cbbb";
    let build = |extra: &str| {
        Manifest::parse(&format!(
            "{HEAD}[kernel_builds.walleye]\ntag = \"int/walleye/ufs-off/v7.3/1\"\nrev = \"{rev}\"\nepoch = 1795421017\nsha256 = \"{sha}\"\n{extra}"
        ))
    };
    assert!(build("").is_ok());
    assert!(build(&format!(
        "configs = [\"{sha}\"]\npatches = [\"{sha}\"]\ntargets = [\"Image.gz\", \"qcom/x.dtb\"]\ndtc_flags = \"-@\"\n"
    ))
    .is_ok());
    for (what, extra) in [
        (
            "a config that is a path",
            "configs = [\"walleye.config\"]\n",
        ),
        ("a patch that is a path", "patches = [\"../p\"]\n"),
        ("no targets", "targets = []\n"),
        ("a target with a space", "targets = [\"Image.gz V=1\"]\n"),
        (
            "an output that leaves the build folder",
            "output = \"../x\"\n",
        ),
        ("a misspelt key", "config = \"x\"\n"),
    ] {
        assert!(build(extra).is_err(), "{what} was accepted");
    }
    let bad = |from: &str, to: &str| {
        Manifest::parse(&format!(
            "{HEAD}[kernel_builds.walleye]\ntag = \"int/walleye/ufs-off/v7.3/1\"\nrev = \"{rev}\"\nepoch = 1795421017\nsha256 = \"{sha}\"\n"
        ).replace(from, to))
    };
    assert!(
        bad(&format!("rev = \"{rev}\""), "rev = \"v7.3\"").is_err(),
        "a rev that is not a commit"
    );
    assert!(
        bad("tag = \"int/walleye/ufs-off/v7.3/1\"", "tag = \"v7.3\"").is_err(),
        "a tag that is not int/"
    );
    assert!(
        bad("[kernel_builds.walleye]", "[kernel_builds.Walleye]").is_err(),
        "a name with a capital"
    );
    assert!(
        bad(&format!("sha256 = \"{sha}\""), "sha256 = \"abc\"").is_err(),
        "a short sha256"
    );
}

#[test]
fn a_kernel_build_refuses_what_git_or_make_would_misread_and_a_name_or_sha_that_means_two_things() {
    let sha = "ab".repeat(32);
    let rev = "0e20c7a89ac5748fbd38f59a2b66324aad84cbbb";
    let row = |name: &str, extra: &str, sha: &str| {
        format!(
            "[kernel_builds.{name}]\ntag = \"int/a/b/v1/1\"\nrev = \"{rev}\"\nepoch = 1\nsha256 = \"{sha}\"\n{extra}"
        )
    };
    let one = |extra: &str| Manifest::parse(&format!("{HEAD}{}", row("k", extra, &sha)));
    for (what, extra) in [
        (
            "a repo with a scheme git does not take here",
            "repo = \"ext::sh -c x\"\n",
        ),
        ("a repo with a space", "repo = \"https://a b/x.git\"\n"),
        (
            "a target that is a make option",
            "targets = [\"-fother\"]\n",
        ),
        ("an absolute output", "output = \"/etc/x\"\n"),
        ("a defconfig with a space", "defconfig = \"a b\"\n"),
        ("dtc flags with a quote", "dtc_flags = \"-@ 'x\"\n"),
    ] {
        assert!(one(extra).is_err(), "{what} was accepted");
    }
    assert!(one("repo = \"https://github.com/linux-phone-oco/linux.git\"\n").is_ok());
    assert!(
        Manifest::parse(&format!(
            "{HEAD}{}",
            row("k", "", &sha).replace("int/a/b/v1/1", "int/a:b")
        ))
        .is_err(),
        "a tag with a colon (a refspec) was accepted"
    );
    // one name, one meaning; one sha256, one row
    let both = format!("{HEAD}[images.k]\npath = \"k.img\"\n{}", row("k", "", &sha));
    let e = Manifest::parse(&both).unwrap_err();
    assert!(e.contains("two meanings"), "{e}");
    let twice = format!("{HEAD}{}{}", row("k", "", &sha), row("j", "", &sha));
    let e = Manifest::parse(&twice).unwrap_err();
    assert!(e.contains("same sha256"), "{e}");
}

// source of the shape: droid-juicer's README (gitlab.com/mobian1/droid-juicer): a config named after the device's DT
// `compatible`, one `[juicer]` section whose `firmware` entries name a partition, an origin folder on it, a destination
// under the extraction folder and files (name, rename optional)
const FIRMWARE: &str = r#"
[layer]
name = "xiaomi-surya"

[layer.firmware]
method = "extract-from-partitions"
compatible = "xiaomi,surya"

[[layer.firmware.source]]
partition = "vendor"
origin = "firmware"
destination = "qcom/sm7150/xiaomi/surya"
files = [{ name = "a615_zap.mdt" }, { name = "wlanmdsp.mbn", rename = "wlanmdsp.otaupdate" }]
"#;

#[test]
fn the_firmware_extraction_is_declared_by_partition_folder_and_file() {
    let m = Manifest::parse(&format!("{HEAD}{FIRMWARE}")).unwrap();
    let f = m.layer.unwrap().firmware.unwrap();
    assert_eq!(f.compatible.as_deref(), Some("xiaomi,surya"));
    assert_eq!(f.source.len(), 1);
    assert_eq!(f.source[0].partition, "vendor");
    assert_eq!(
        f.source[0].files[1].rename.as_deref(),
        Some("wlanmdsp.otaupdate")
    );
}

#[test]
fn a_firmware_declaration_that_would_break_the_extractors_toml_or_leave_its_folder_is_refused() {
    for (from, to) in [
        ("partition = \"vendor\"", "partition = \"ven\\\"dor\""),
        ("partition = \"vendor\"", "partition = \"..\""),
        (
            "{ name = \"a615_zap.mdt\" }",
            "{ name = \"a615_zap.mdt\", rename = \".\" }",
        ),
        ("origin = \"firmware\"", "origin = \"../firmware\""),
        ("origin = \"firmware\"", "origin = \"/firmware\""),
        (
            "destination = \"qcom/sm7150/xiaomi/surya\"",
            "destination = \"/lib/firmware\"",
        ),
        ("{ name = \"a615_zap.mdt\" }", "{ name = \"a/b.mdt\" }"),
        ("{ name = \"a615_zap.mdt\" }", "{ name = \"\" }"),
        (
            "compatible = \"xiaomi,surya\"",
            "compatible = \"xiaomi surya\"",
        ),
        ("compatible = \"xiaomi,surya\"", "compatible = \"../x\""),
    ] {
        let t = format!("{HEAD}{}", FIRMWARE.replace(from, to));
        assert!(Manifest::parse(&t).is_err(), "{to} was accepted");
    }
    // sources without a compatible: the extractor's config has no name
    let t = format!(
        "{HEAD}{}",
        FIRMWARE.replace("compatible = \"xiaomi,surya\"\n", "")
    );
    assert!(Manifest::parse(&t).unwrap_err().contains("compatible"));
    // a vendor blob is never a layer file: a destination under /usr/lib/firmware is refused
    let t = format!(
        "{HEAD}{FIRMWARE}\n[[layer.file]]\nfrom = \"fw/a.mbn\"\nto = \"/usr/lib/firmware/qcom/a.mbn\"\nkind = \"data\"\n"
    );
    assert!(Manifest::parse(&t).unwrap_err().contains("firmware"));
}

/// `[part_builds.NAME]` (item recipe-static-binary-part): a static aarch64 binary made from a crate at a commit in
/// the sandbox. The repository and the commit name what is read, the crate's folder and the binary what is built,
/// the sha256 what it must be.
#[test]
fn a_part_build_names_its_repository_commit_crate_and_binary_and_the_sha256_it_must_have() {
    let sha = "ab".repeat(32);
    let rev = "0e20c7a89ac5748fbd38f59a2b66324aad84cbbb";
    let row = |name: &str, extra: &str, sha: &str| {
        format!(
            "[part_builds.{name}]\nrepo = \"https://github.com/linux-phone-oco/google-walleye-lab.git\"\nrev = \"{rev}\"\nbin = \"walleye-reboot\"\nsha256 = \"{sha}\"\n{extra}"
        )
    };
    let one = |extra: &str| Manifest::parse(&format!("{HEAD}{}", row("p", extra, &sha)));
    assert!(one("").is_ok());
    assert!(one("crate = \"tools/walleye-reboot\"\n").is_ok());
    for (what, extra) in [
        ("an absolute crate folder", "crate = \"/tools/x\"\n"),
        ("a crate folder that leaves the repo", "crate = \"../x\"\n"),
        ("a crate folder with a dot segment", "crate = \"a/./b\"\n"),
        ("a misspelt key", "target = \"x\"\n"),
    ] {
        assert!(one(extra).is_err(), "{what} was accepted");
    }
    let bad = |from: &str, to: &str| {
        Manifest::parse(&format!("{HEAD}{}", row("p", "", &sha).replace(from, to)))
    };
    assert!(
        bad(&format!("rev = \"{rev}\""), "rev = \"main\"").is_err(),
        "a rev that is not a commit"
    );
    assert!(
        bad("repo = \"https://", "repo = \"ext::sh -c x\" #").is_err(),
        "a repo with a scheme git does not take here"
    );
    assert!(
        bad("bin = \"walleye-reboot\"", "bin = \"-x\"").is_err(),
        "a binary named like a cargo option"
    );
    assert!(
        bad("[part_builds.p]", "[part_builds.P]").is_err(),
        "a name with a capital"
    );
    assert!(
        bad(&format!("sha256 = \"{sha}\""), "sha256 = \"abc\"").is_err(),
        "a short sha256"
    );
    // one name, one meaning; one sha256, one row
    let both = format!("{HEAD}[images.p]\npath = \"p.img\"\n{}", row("p", "", &sha));
    let e = Manifest::parse(&both).unwrap_err();
    assert!(e.contains("two meanings"), "{e}");
    let twice = format!("{HEAD}{}{}", row("p", "", &sha), row("q", "", &sha));
    let e = Manifest::parse(&twice).unwrap_err();
    assert!(e.contains("same sha256"), "{e}");
}
