// SPDX-License-Identifier: MIT OR Apache-2.0
//! What osfact reads from a device.toml - [device] arch and [install.connection], [install.root], [install.way_back] -
//! the manifest reads too, from osfact's own types (item manifest-device-arch): the one definition.

use lpoco_manifest::{schema, Manifest};

const FIXTURE: &str = include_str!("fixtures/install-tables.toml");

#[test]
fn the_arch_and_the_install_tables_osfact_reads_are_read() {
    let m = Manifest::parse(FIXTURE).unwrap();
    assert_eq!(m.device.unwrap().arch.unwrap().as_str(), "aarch64");
    let i = m.install.unwrap();
    assert_eq!(i.connection.unwrap().shell_port, 23);
    assert_eq!(i.root.unwrap().path, "/mm");
    assert!(i.way_back.unwrap().command.starts_with("sync;"));
}

#[test]
fn a_misspelled_arch_is_refused_naming_it() {
    let e = Manifest::parse(&FIXTURE.replace("aarch64", "arm65")).unwrap_err();
    assert!(e.contains("arm65"), "the refusal names the value: {e}");
}

#[test]
fn a_misspelled_install_key_is_refused_naming_it() {
    for (from, to) in [
        ("shell_port", "shell_prot"),
        ("size_mib", "size_mb"),
        ("[install.way_back]", "[install.wayback]"),
    ] {
        let e = Manifest::parse(&FIXTURE.replace(from, to)).unwrap_err();
        assert!(e.contains(to), "the refusal names {to}: {e}");
    }
}

#[test]
fn the_generated_schema_names_them() {
    let s = schema();
    for word in [
        "\"arch\"",
        "\"install\"",
        "\"way_back\"",
        "\"shell_port\"",
        "\"size_mib\"",
    ] {
        assert!(s.contains(word), "the schema lacks {word}");
    }
}
