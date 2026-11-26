// SPDX-License-Identifier: MIT OR Apache-2.0
//! A phone that boots more than one system declares a connection for each (decision
//! 2026-11-26-a-phone-that-boots-more-than-one-system-declares): [connection] is the default, [connections.NAME]
//! the others, of the same shape; what cannot work is refused naming the connection.

use lpoco_manifest::{Login, Manifest, Transport};

const HEAD: &str = "[device]\ncodename = \"google-walleye\"\n[connection]\ntransport = \"telnet\"\nuser = \"root\"\naddresses = [\"172.16.42.1\"]\nshell_port = 23\n";

fn parse(connections: &str) -> Result<Manifest, String> {
    Manifest::parse(&format!("{HEAD}{connections}"))
}

#[test]
fn a_device_with_one_connection_has_no_named_ones() {
    let m = parse("").unwrap();
    assert!(m.connections.is_empty());
}

#[test]
fn an_adb_connection_beside_the_telnet_default_parses_and_keeps_the_default() {
    let m = parse("[connections.android]\ntransport = \"adb\"\nuser = \"shell\"\naddresses = []\n")
        .unwrap();
    assert_eq!(m.connection.unwrap().transport, Transport::Telnet);
    let android = &m.connections["android"];
    assert_eq!(
        (android.transport, android.login()),
        (Transport::Adb, Login::None)
    );
}

#[test]
fn an_unknown_key_in_a_named_connection_is_refused_naming_it() {
    let e = parse("[connections.android]\ntransport = \"adb\"\nuser = \"shell\"\naddresses = []\nserial = \"X\"\n").unwrap_err();
    assert!(e.contains("serial"), "{e}");
}

#[test]
fn a_named_connection_that_cannot_work_is_refused_naming_it() {
    let e =
        parse("[connections.recovery]\ntransport = \"telnet\"\nuser = \"root\"\naddresses = []\n")
            .unwrap_err();
    assert!(
        e.contains("connections.recovery") && e.contains("shell_port"),
        "{e}"
    );
}

#[test]
fn a_connection_name_is_a_plain_word() {
    for name in ["\"\"", "\"an droid\"", "\"a/b\"", "\"-x\""] {
        let e = parse(&format!(
            "[connections.{name}]\ntransport = \"adb\"\nuser = \"s\"\naddresses = []\n"
        ))
        .unwrap_err();
        assert!(e.contains("connection name"), "{name}: {e}");
    }
}

#[test]
fn the_schema_carries_the_named_connections() {
    let schema = lpoco_manifest::schema();
    assert!(schema.contains("\"connections\""), "{schema}");
}
