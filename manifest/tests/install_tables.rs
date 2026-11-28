// SPDX-License-Identifier: MIT OR Apache-2.0
//! What osfact owns in a device.toml - [device] arch and serial_env, the [install] tables - lpoco-manifest's reader
//! keeps unread (decision 2026-11-28-osfact-s-tables-in-device-toml-are-read-and): osfact alone reads and checks
//! them (`osfact::declaration::check`, called by `lpoco-lab manifest check`), so a key osfact adds after this
//! release needs no manifest release. The generated schema still embeds osfact's types.

use lpoco_manifest::{schema, Manifest};

const FIXTURE: &str = include_str!("fixtures/install-tables.toml");

/// A key that osfact adds after this release: no type of lpoco-manifest has ever seen it.
fn with_future_keys() -> String {
    FIXTURE
        .replace(
            "arch = \"aarch64\"",
            "arch = \"aarch64\"\nserial_env = \"WALLEYE_SERIAL\"\nfuture_device_key = 1",
        )
        .replace(
            "size_mib = 400",
            "size_mib = 400\nfuture_root_key = \"x\"\n[install.future_table]\nk = 1",
        )
}

#[test]
fn a_key_osfact_adds_later_is_read_without_error() {
    let m = Manifest::parse(&with_future_keys()).unwrap();
    assert_eq!(m.device.unwrap().codename, "google-walleye");
    assert!(m.install.is_some(), "the install tables are kept, unread");
}

#[test]
fn a_misspelled_osfact_key_is_not_this_readers_to_refuse() {
    // osfact refuses these (osfact/tests/declaration_check.rs, devlab/tests/manifest_check.rs); here they pass
    for (from, to) in [
        ("shell_port", "shell_prot"),
        ("binds", "bindz"),
        ("aarch64", "arm65"),
    ] {
        Manifest::parse(&FIXTURE.replace(from, to))
            .unwrap_or_else(|e| panic!("{to} is refused by the manifest reader: {e}"));
    }
}

#[test]
fn the_generated_schema_still_names_osfacts_keys() {
    let s = schema();
    for word in [
        "\"arch\"",
        "\"serial_env\"",
        "\"install\"",
        "\"way_back\"",
        "\"back_on\"",
        "\"shell_port\"",
        "\"size_mib\"",
        "\"check_wait\"",
        "\"binds\"",
        "\"aarch64\"",
    ] {
        assert!(s.contains(word), "the schema lacks {word}");
    }
}

/// The keys of [device] that osfact reads (its fixture osfact/tests/fixtures/google-walleye/device.toml): the
/// manifest reader accepts each, whatever osfact's types are.
#[test]
fn every_device_key_osfact_pack_reads_is_accepted() {
    const OSFACT_FIXTURE: &str =
        include_str!("../../osfact/tests/fixtures/google-walleye/device.toml");
    let device: toml::Table = toml::from_str(OSFACT_FIXTURE).unwrap();
    let keys: Vec<&String> = device["device"].as_table().unwrap().keys().collect();
    assert!(
        keys.iter().any(|k| *k == "serial_env"),
        "the fixture names serial_env: {keys:?}"
    );
    for key in keys.into_iter().filter(|k| *k != "codename") {
        let text = format!(
            "[device]\ncodename = \"google-walleye\"\n{key} = {}\n\n[connection]\nuser = \"u\"\naddresses = [\"a\"]\n",
            device["device"][key.as_str()]
        );
        Manifest::parse(&text).unwrap_or_else(|e| panic!("[device] {key} is refused: {e}"));
    }
}
