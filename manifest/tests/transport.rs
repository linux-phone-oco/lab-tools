// SPDX-License-Identifier: MIT OR Apache-2.0
//! How the phone is reached is declared (decision 2026-11-19-phone-reached-by-its-declared-transport):
//! transport ssh | telnet | adb, login password | none, and what cannot be reached is refused.

use lpoco_manifest::{Login, Manifest, Transport};

const HEAD: &str = "[device]\ncodename = \"p\"\n";

fn parse(connection: &str) -> Result<Manifest, String> {
    Manifest::parse(&format!(
        "{HEAD}[connection]\nuser = \"u\"\naddresses = [\"10.0.0.1\"]\n{connection}"
    ))
}

#[test]
fn a_device_that_says_nothing_is_reached_by_ssh_with_a_password() {
    let c = parse("").unwrap().connection.unwrap();
    assert_eq!(c.transport, Transport::Ssh);
    assert_eq!(c.login(), Login::Password);
}

#[test]
fn telnet_and_adb_are_reached_with_no_login_unless_said() {
    let c = parse("transport = \"telnet\"\nshell_port = 23\n")
        .unwrap()
        .connection
        .unwrap();
    assert_eq!((c.transport, c.login()), (Transport::Telnet, Login::None));
    let c = parse("transport = \"adb\"\nlogin = \"none\"\n")
        .unwrap()
        .connection
        .unwrap();
    assert_eq!((c.transport, c.login()), (Transport::Adb, Login::None));
}

#[test]
fn an_unknown_transport_or_login_is_refused_naming_it() {
    for (text, word) in [
        ("transport = \"carrier-pigeon\"\n", "carrier-pigeon"),
        ("login = \"fingerprint\"\n", "fingerprint"),
    ] {
        let e = parse(text).unwrap_err();
        assert!(e.contains(word), "{text}: {e}");
    }
}

#[test]
fn a_transport_that_cannot_work_as_declared_is_refused() {
    for (text, word) in [
        ("transport = \"telnet\"\n", "shell_port"),
        ("transport = \"ssh\"\nlogin = \"none\"\n", "ssh"),
        ("transport = \"adb\"\nlogin = \"password\"\n", "adb"),
    ] {
        let e = parse(text).unwrap_err();
        assert!(e.contains("connection") && e.contains(word), "{text}: {e}");
    }
}

#[test]
fn an_ssh_phone_may_be_reached_by_a_key_whose_identity_path_is_declared() {
    let c = parse("login = \"key\"\nidentity = \"/home/lab/.config/devlab/p/id_ed25519\"\n")
        .unwrap()
        .connection
        .unwrap();
    assert_eq!((c.transport, c.login()), (Transport::Ssh, Login::Key));
    assert_eq!(
        c.identity.as_deref(),
        Some("/home/lab/.config/devlab/p/id_ed25519")
    );
}

#[test]
fn a_key_login_without_an_identity_and_an_identity_without_a_key_login_are_refused() {
    for (text, words) in [
        ("login = \"key\"\n", ["connection", "identity"]),
        ("identity = \"/k\"\n", ["connection", "identity"]),
        (
            "transport = \"adb\"\nlogin = \"none\"\nidentity = \"/k\"\n",
            ["connection", "identity"],
        ),
        (
            "transport = \"adb\"\nlogin = \"key\"\nidentity = \"/k\"\n",
            ["connection", "adb"],
        ),
    ] {
        let e = parse(text).unwrap_err();
        assert!(words.iter().all(|w| e.contains(w)), "{text}: {e}");
    }
}

#[test]
fn an_identity_that_is_empty_or_not_absolute_is_refused() {
    for id in ["", "~/.ssh/id", "id_key"] {
        let e = parse(&format!("login = \"key\"\nidentity = \"{id}\"\n")).unwrap_err();
        assert!(
            e.contains("connection") && e.contains("identity"),
            "{id}: {e}"
        );
    }
}
