// SPDX-License-Identifier: MIT OR Apache-2.0
//! The keys attended procedures read have homes (decision 2026-11-19-attended-keys-in-the-manifest): each fact in
//! the table that owns it, named images in [images], a procedure's own values in [procedures.NAME].

use lpoco_manifest::Manifest;

const HEAD: &str = "[device]\ncodename = \"walleye\"\n";

/// Every key of walleye's list (walleye-to-orchestration-manifest-keys.md), each in its home.
const WALLEYE: &str = r#"
[connection]
user = "root"
addresses = ["10.0.0.1"]
shell_port = 23
shell_address = "10.0.0.2"

[tools]
lpoco = "0.31.1"
adb = "/usr/bin/adb"
fastboot = "/usr/bin/fastboot"

[health]
battery_floor_mv = 3700
battery_source = "fastboot-getvar"

[boot]
cmdline = "x"
kernel_offset = 0
ramdisk_offset = 0
tags_offset = 0
pagesize = 4096
device_wait = 170
back_wait = 240

[images]
dir = "images"

[images.z21]
path = "images-z21/boot-z21.img"
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"

[images.pmic-a]
path = "images-wdt1/boot-wdt1-a.img"

[procedures.touch-try]
window = 120
min_window = 40
stay = 180
bite_wait = 150
note = "free words too"
"#;

fn parse(extra: &str) -> Result<Manifest, String> {
    Manifest::parse(&format!("{HEAD}{extra}"))
}

#[test]
fn walleyes_whole_list_has_homes() {
    let m = parse(WALLEYE).unwrap();
    assert_eq!(m.connection.as_ref().unwrap().shell_port, Some(23));
    assert_eq!(
        m.tools.as_ref().unwrap().adb.as_deref(),
        Some("/usr/bin/adb")
    );
    assert_eq!(m.health.battery_floor_mv, Some(3700));
    assert_eq!(m.boot.as_ref().unwrap().back_wait, Some(240));
    let i = m.images.as_ref().unwrap();
    assert_eq!(i.dir.as_deref(), Some("images"));
    assert_eq!(i.named["z21"].path, "images-z21/boot-z21.img");
    assert!(m.procedures.contains_key("touch-try"));
}

#[test]
fn a_misspelt_key_in_each_new_home_is_refused() {
    for (typo, word) in [
        ("[tools]\nadbb = \"x\"\n", "adbb"),
        ("[health]\nbattery_flor_mv = 1\n", "battery_flor_mv"),
        ("[images.a]\npath = \"x\"\nsha265 = \"y\"\n", "sha265"),
        ("[images]\ndri = \"x\"\n", ""),
    ] {
        let e = parse(typo).unwrap_err();
        assert!(word.is_empty() || e.contains(word), "{typo}: {e}");
    }
    let e = parse("[connection]\nuser = \"u\"\naddresses = []\nshell_prot = 1\n").unwrap_err();
    assert!(e.contains("shell_prot"), "{e}");
}

#[test]
fn an_image_that_cannot_be_trusted_is_refused_naming_it() {
    for (img, why) in [
        ("path = \"/abs/boot.img\"", "relative"),
        ("path = \"../boot.img\"", "relative"),
        ("path = \"\"", "path"),
        ("path = \"a.img\"\nsha256 = \"abc\"", "sha256"),
    ] {
        let e = parse(&format!("[images.bad]\n{img}\n")).unwrap_err();
        assert!(e.contains("images.bad") && e.contains(why), "{img}: {e}");
    }
    let e = parse("[images.Bad Name]\npath = \"a\"\n").unwrap_err();
    assert!(!e.is_empty());
}

#[test]
fn a_battery_floor_without_a_way_to_read_it_and_a_source_without_a_floor_are_refused() {
    assert!(parse("[health]\nbattery_source = \"fastboot-getvar\"\n").is_err());
    assert!(parse("[health]\nbattery_floor_mv = 3700\n").is_err());
}

#[test]
fn a_procedures_table_holds_numbers_and_words_only() {
    assert!(parse("[procedures.t]\nwindow = 1.5\nname = \"x\"\n").is_ok());
    for bad in ["flag = true", "list = [1]", "sub = { a = 1 }"] {
        let e = parse(&format!("[procedures.t]\n{bad}\n")).unwrap_err();
        assert!(!e.is_empty(), "{bad}");
    }
}

#[test]
fn a_procedure_key_a_dotted_read_could_not_reach_is_refused() {
    let e = parse("[procedures.t]\n\"a.b\" = 1\n").unwrap_err();
    assert!(e.contains("procedures.t") && e.contains("a.b"), "{e}");
}
