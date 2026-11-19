// SPDX-License-Identifier: MIT OR Apache-2.0
//! A stale phone-side piece is named by `lpoco host check`, from the phone's last gathered report - no phone.
use lpoco_host::{blob_id, parse_device, stale_pieces, Stale};
use std::path::Path;

const MANIFEST: &str = r#"
[device]
codename = "xiaomi-surya"
[tools]
lpoco = "0.9"
[[flash.overlay]]
from = "lab/devlab-usb-awake"
to = "root:/usr/local/bin/devlab-usb-awake"
mode = "755"
"#;
const ON_MAIN: &[u8] = b"#!/bin/sh\n# held always, with the bt gate\n";
const ON_PHONE: &[u8] = b"#!/bin/sh\n# held only while configured\n";

fn report(piece_blob: &str, health_version: &str) -> String {
    format!(
        "{{\"layer\":\"base\",\"component\":\"lab-pieces\",\"test\":\"piece-file:/usr/local/bin/devlab-usb-awake\",\"outcome\":\"pass\",\"detail\":\"/usr/local/bin/devlab-usb-awake {piece_blob}\"}}\n\
         {{\"layer\":\"base\",\"component\":\"lab-pieces\",\"test\":\"piece-package-lpoco-health\",\"outcome\":\"pass\",\"detail\":\"lpoco-health {health_version}-r0\"}}\n"
    )
}

#[test]
fn check_names_a_stale_phone_piece() {
    let device = parse_device(Path::new("device.toml"), MANIFEST).unwrap();
    let main = |from: &str| (from == "lab/devlab-usb-awake").then(|| ON_MAIN.to_vec());
    let stale = stale_pieces(&device, &report(&blob_id(ON_PHONE), "0.9.1"), main).unwrap();
    assert_eq!(
        stale,
        vec![Stale::File {
            path: "/usr/local/bin/devlab-usb-awake".into(),
            found: blob_id(ON_PHONE),
            main: blob_id(ON_MAIN),
        }]
    );
}

#[test]
fn check_names_a_package_off_its_pin_and_passes_a_current_phone() {
    let device = parse_device(Path::new("device.toml"), MANIFEST).unwrap();
    let main = |_: &str| Some(ON_MAIN.to_vec());
    let off = stale_pieces(&device, &report(&blob_id(ON_MAIN), "0.8.0"), main).unwrap();
    assert_eq!(
        off,
        vec![Stale::Package {
            name: "lpoco-health".into(),
            found: "0.8.0".into(),
            pin: "0.9".into()
        }]
    );
    assert!(
        stale_pieces(&device, &report(&blob_id(ON_MAIN), "0.9.1"), main)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_missing_piece_is_named_and_a_line_that_is_no_row_is_an_error() {
    let device = parse_device(Path::new("device.toml"), MANIFEST).unwrap();
    let main = |_: &str| Some(ON_MAIN.to_vec());
    let gone = stale_pieces(&device, &report("missing", "0.9.1"), main).unwrap();
    assert_eq!(
        gone,
        vec![Stale::Missing {
            what: "/usr/local/bin/devlab-usb-awake".into()
        }]
    );
    assert!(stale_pieces(&device, "not json\n", main).is_err());
}

/// What the lab prints at a change's end: the report it just read back, judged against a device repository's
/// origin/main, one line a stale piece - no phone, a real git repository.
#[test]
fn the_lab_names_a_piece_off_main_from_the_report_it_reads_back() {
    use lpoco_host::stale_lines;
    use std::process::Command;
    let dir = std::env::temp_dir().join(format!("lpoco-pieces-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("lab")).unwrap();
    std::fs::write(dir.join("lab/devlab-usb-awake"), ON_PHONE).unwrap();
    let git = |args: &[&str]| {
        let o = Command::new("git")
            .args(["-C", dir.to_str().unwrap()])
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();
        assert!(o.status.success(), "git {args:?}: {o:?}");
    };
    git(&["init", "-q"]);
    git(&["add", "."]);
    git(&["commit", "-qm", "the old piece"]);
    std::fs::write(dir.join("lab/devlab-usb-awake"), ON_MAIN).unwrap();
    git(&["commit", "-qam", "the new piece"]);
    git(&["update-ref", "refs/remotes/origin/main", "HEAD"]);
    let device = parse_device(&dir.join("device.toml"), MANIFEST).unwrap();

    let lines = stale_lines(&device, &report(&blob_id(ON_PHONE), "0.9.1"), "the run").unwrap();
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].contains("xiaomi-surya phone piece /usr/local/bin/devlab-usb-awake: STALE"));
    assert!(
        lines[0].contains("the old piece"),
        "names the commit: {lines:?}"
    );
    assert!(lines[0].contains("(per the run)"));

    let ok = stale_lines(&device, &report(&blob_id(ON_MAIN), "0.9.1"), "the run").unwrap();
    assert_eq!(
        ok,
        vec!["xiaomi-surya phone pieces: ok, as main per the run"]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_lines_name_a_package_off_its_pin() {
    use lpoco_host::stale_lines;
    let device = parse_device(Path::new("/nonexistent/device.toml"), MANIFEST).unwrap();
    // no repository: main gives nothing, so a file is not judged, but the package is
    let gone = stale_lines(&device, &report(&blob_id(ON_MAIN), "0.8.0"), "r").unwrap();
    assert!(
        gone.iter()
            .any(|l| l.contains("phone package lpoco-health: STALE, 0.8.0 is off the pin 0.9")),
        "{gone:?}"
    );
    assert!(stale_lines(&device, "not json\n", "r").is_err());
}

const CONFIGS: &str = r#"
[device]
codename = "xiaomi-surya"
[[user_config]]
path = "/home/user/.config/sxmo/hooks/sxmo_hook_lock.sh"
expect = "/usr/share/sxmo/default_hooks/sxmo_hook_lock.sh"
[[user_config]]
path = "/home/user/.config/sxmo/profile"
from = "config/sxmo/profile"
"#;
const HOOK: &str = "/home/user/.config/sxmo/hooks/sxmo_hook_lock.sh";
const PROFILE: &str = "/home/user/.config/sxmo/profile";
const PACKAGE_HOOK: &[u8] = b"#!/bin/sh\n# as the package ships it\n";
const EDITED_HOOK: &[u8] = b"#!/bin/sh\n# edited on the phone\n";
const PROFILE_IN_BASE: &[u8] = b"export X=1\n";
const PROFILE_ON_PHONE: &[u8] = b"export X=2\n";

/// The rows health's 94-lab-pieces.sh writes for a declared user config: `config-file:PATH`, detail
/// `PATH BLOB DEFAULT` - the file's blob, then the blob of the package default it is declared against
/// (`-` when it is declared against the base's copy), `missing` for a file the phone does not hold.
fn config_report(hook: &str, hook_default: &str, profile: &str) -> String {
    format!(
        "{{\"layer\":\"base\",\"component\":\"lab-pieces\",\"test\":\"config-file:{HOOK}\",\"outcome\":\"pass\",\"detail\":\"{HOOK} {hook} {hook_default}\"}}\n\
         {{\"layer\":\"base\",\"component\":\"lab-pieces\",\"test\":\"config-file:{PROFILE}\",\"outcome\":\"pass\",\"detail\":\"{PROFILE} {profile} -\"}}\n\
         {{\"layer\":\"base\",\"component\":\"lab-pieces\",\"test\":\"config-file:/home/user/.config/other\",\"outcome\":\"pass\",\"detail\":\"/home/user/.config/other {} -\"}}\n",
        blob_id(b"nobody declared me")
    )
}

#[test]
fn a_user_config_off_its_package_default_or_its_base_copy_is_named_and_a_matching_one_passes() {
    let device = parse_device(Path::new("device.toml"), CONFIGS).unwrap();
    let main = |from: &str| (from == "config/sxmo/profile").then(|| PROFILE_IN_BASE.to_vec());
    let (edited, shipped) = (blob_id(EDITED_HOOK), blob_id(PACKAGE_HOOK));
    let (on_phone, in_base) = (blob_id(PROFILE_ON_PHONE), blob_id(PROFILE_IN_BASE));

    let stale = stale_pieces(&device, &config_report(&edited, &shipped, &on_phone), main).unwrap();
    assert_eq!(
        stale,
        vec![
            Stale::Config {
                path: HOOK.into(),
                found: edited.clone(),
                expected: shipped.clone(),
                against: "the package default /usr/share/sxmo/default_hooks/sxmo_hook_lock.sh"
                    .into(),
            },
            Stale::Config {
                path: PROFILE.into(),
                found: on_phone,
                expected: in_base.clone(),
                against: "the base's copy config/sxmo/profile".into(),
            },
        ]
    );
    // both as declared: nothing named; an undeclared file in the report is never read
    assert!(
        stale_pieces(&device, &config_report(&shipped, &shipped, &in_base), main)
            .unwrap()
            .is_empty()
    );
    // a file the phone does not hold has no edit to lose: not named (the package default applies)
    assert!(
        stale_pieces(&device, &config_report("missing", &shipped, &in_base), main)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_user_config_must_be_declared_against_exactly_one_thing() {
    for body in [
        "path = \"/p\"\n",
        "path = \"/p\"\nexpect = \"/e\"\nfrom = \"f\"\n",
        "path = \"relative\"\nexpect = \"/e\"\n",
    ] {
        let text = format!("[device]\ncodename = \"x\"\n[[user_config]]\n{body}");
        assert!(
            parse_device(Path::new("device.toml"), &text).is_err(),
            "{body}"
        );
    }
}

#[test]
fn a_config_whose_default_or_base_copy_cannot_be_found_is_named_not_passed() {
    let device = parse_device(Path::new("device.toml"), CONFIGS).unwrap();
    let shipped = blob_id(PACKAGE_HOOK);
    // the package no longer ships its default; the base does not hold the declared copy; the phone lacks it
    let none = |_: &str| None;
    let stale = stale_pieces(
        &device,
        &config_report(&shipped, "missing", "missing"),
        none,
    )
    .unwrap();
    assert_eq!(stale.len(), 2, "{stale:?}");
    assert!(
        stale.iter().all(|s| matches!(s, Stale::Missing { .. })),
        "{stale:?}"
    );
}

#[test]
fn a_user_config_declared_twice_against_itself_or_out_of_the_base_is_refused() {
    for body in [
        "path = \"/p\"\nexpect = \"/e\"\n[[user_config]]\npath = \"/p\"\nfrom = \"f\"\n",
        "path = \"/p\"\nexpect = \"/p\"\n",
        "path = \"/p\"\nfrom = \"../f\"\n",
        "path = \"/p\"\nfrom = \"/abs\"\n",
    ] {
        let text = format!("[device]\ncodename = \"x\"\n[[user_config]]\n{body}");
        assert!(
            parse_device(Path::new("device.toml"), &text).is_err(),
            "{body}"
        );
    }
}
