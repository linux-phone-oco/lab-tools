// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-lab manifest check --device FILE` (backlog device-toml-checked-against-host): a device repo's CI asks
//! the host's own reader whether it can read device.toml - what every command reads, parsed and validated only,
//! so no state directory, no phone and no lease is touched.

use std::path::{Path, PathBuf};
use std::process::Command;

const CONNECTED: &str =
    "[device]\ncodename = \"x\"\n[connection]\nuser = \"u\"\naddresses = [\"10.0.0.1\"]\n";

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Scratch {
        let d = std::env::temp_dir().join(format!(
            "lpoco-manifest-check-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home")).unwrap();
        Scratch(d)
    }
    fn file(&self, text: &str) -> PathBuf {
        let f = self.0.join("device.toml");
        std::fs::write(&f, text).unwrap();
        f
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(s: &Scratch, args: &[&Path]) -> (i32, String, String) {
    let o = Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
        .args(["manifest", "check"])
        .args(args)
        .current_dir(&s.0)
        .env("HOME", s.0.join("home"))
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .output()
        .unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn with_device(s: &Scratch, f: &Path) -> (i32, String, String) {
    check(s, &[Path::new("--device"), f])
}

#[test]
fn a_manifest_the_host_reads_passes_and_leaves_no_trace() {
    let s = Scratch::new("ok");
    let f = s.file(CONNECTED);
    let (code, out, err) = with_device(&s, &f);
    assert_eq!(code, 0, "{out}{err}");
    assert!(
        out.contains("device.toml") && out.contains(env!("CARGO_PKG_VERSION")),
        "{out}"
    );
    let left: Vec<_> = std::fs::read_dir(s.0.join("home")).unwrap().collect();
    assert!(left.is_empty(), "the check wrote into HOME: {left:?}");
}

#[test]
fn the_manifest_is_found_above_when_no_device_is_given() {
    let s = Scratch::new("above");
    s.file(CONNECTED);
    let (code, out, err) = check(&s, &[]);
    assert_eq!(code, 0, "{out}{err}");
}

#[test]
fn a_key_the_host_cannot_read_fails_naming_file_key_and_version() {
    let s = Scratch::new("bad");
    let f = s.file(&format!("{CONNECTED}[nonesuch]\na = 1\n"));
    let (code, out, err) = with_device(&s, &f);
    assert_eq!(code, 1, "{err}");
    assert!(out.is_empty(), "{out}");
    assert!(
        err.contains("device.toml") && err.contains("nonesuch"),
        "{err}"
    );
    assert!(err.contains(env!("CARGO_PKG_VERSION")), "{err}");
}

#[test]
fn a_value_the_host_refuses_fails_with_the_reason() {
    let s = Scratch::new("refused");
    let f = s.file(&format!("{CONNECTED}[kernels]\ntest = [\"A b\"]\n"));
    let (code, _, err) = with_device(&s, &f);
    assert_eq!(code, 1, "{err}");
    assert!(
        err.contains("device.toml") && err.contains("is not a kernel name"),
        "{err}"
    );
}

#[test]
fn a_manifest_the_commands_would_refuse_is_refused_here_too() {
    let s = Scratch::new("unconnected");
    let f = s.file("[device]\ncodename = \"x\"\n");
    let (code, _, err) = with_device(&s, &f);
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("[connection] is missing"), "{err}");
}

#[test]
fn a_missing_file_fails_naming_it() {
    let s = Scratch::new("missing");
    let (code, _, err) = with_device(&s, &s.0.join("absent.toml"));
    assert_eq!(code, 1, "{err}");
    assert!(err.contains("absent.toml"), "{err}");
}
