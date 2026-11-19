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
        ("login = \"key\"\n", "key"),
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
