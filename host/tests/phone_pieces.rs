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
