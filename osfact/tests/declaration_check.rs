// SPDX-License-Identifier: MIT OR Apache-2.0
//! `osfact::declaration::check`: osfact alone reads and checks the tables it owns in a device.toml (decision
//! 2026-11-28-osfact-s-tables-in-device-toml-are-read-and). The text is osfact's own fixture, a walleye declaration
//! whose lab tables are not osfact's and are not read.

use osfact::declaration::check;

const FIXTURE: &str = include_str!("fixtures/google-walleye/device.toml");

#[test]
fn the_fixture_declaration_passes() {
    check(FIXTURE).unwrap();
}

#[test]
fn a_misspelled_key_of_each_osfact_table_is_refused_naming_it() {
    for (from, to) in [
        ("shell_port", "shell_prot"),
        ("size_mib", "size_mb"),
        ("back_on", "back_no"),
        ("serial_env", "serial_var"),
        ("[install.way_back]", "[install.wayback]"),
    ] {
        assert!(FIXTURE.contains(from), "the fixture has no {from}");
        let e = check(&FIXTURE.replace(from, to)).unwrap_err();
        assert!(e.contains(to), "the refusal names {to}: {e}");
    }
}

#[test]
fn a_misspelled_arch_is_refused_naming_it() {
    let e = check(&FIXTURE.replace("\"aarch64\"", "\"arm65\"")).unwrap_err();
    assert!(e.contains("arm65"), "{e}");
}

#[test]
fn a_table_that_is_the_labs_is_not_read() {
    check(&format!("{FIXTURE}\n[lab_only]\nanything = 1\n")).unwrap();
}
