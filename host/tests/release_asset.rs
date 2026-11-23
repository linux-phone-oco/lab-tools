// SPDX-License-Identifier: MIT OR Apache-2.0
//! The family's x86_64 programs come from the release's assets (decision 2026-11-19-host-follows-releases (a)):
//! prepare fetches NAME-x86_64-unknown-linux-gnu and its .sha256 from the tag's release, installs the file only
//! when the hash matches, refuses a wrong one, and builds with cargo when the release has no asset. The release
//! is a folder (`LPOCO_RELEASE_BASE=file://DIR`), curl a stub that copies from it, cargo and root recording stubs.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const FAMILY: &[&str] = &[
    "lpoco",
    "lpoco-lab",
    "lpoco-bringup",
    "lpoco-health",
    "lpoco-host",
    "lpoco-tone",
];
const SUFFIX: &str = "x86_64-unknown-linux-gnu";

struct Lab {
    dir: PathBuf,
    release: PathBuf,
    calls: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn lab(name: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("lpoco-host-asset-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let stubs = dir.join("stubs");
    let release = dir.join("release");
    fs::create_dir_all(&stubs).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(&release).unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/none/*/device.toml\"\n[tools]\nlpoco = \"0.2.0\"\n",
            dir.display()
        ),
    )
    .unwrap();
    let calls = dir.join("calls");
    let rec = format!("echo \"$0 $@\" >> {}", calls.display());
    exe(&stubs.join("root"), &rec);
    exe(&stubs.join("cargo"), &rec);
    exe(&stubs.join("git"), &rec);
    exe(&stubs.join("systemctl"), &rec);
    // curl --fail --silent --show-error --location URL --output PATH: a file:// URL is copied, a missing one fails as a 404 does
    exe(
        &stubs.join("curl"),
        &format!(
            "echo \"$0 $@\" >> {}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; --*) ;; *) url=\"$1\";; esac; shift; done\nsrc=\"${{url#file://}}\"\n[ -f \"$src\" ] || {{ printf 404; exit 22; }}\nprintf 200\ncp \"$src\" \"$out\"",
            calls.display()
        ),
    );
    // the stub copies with the real cp, the only program on PATH besides the stubs
    std::os::unix::fs::symlink(which("cp"), stubs.join("cp")).unwrap();
    Lab {
        dir,
        release,
        calls,
    }
}

fn which(p: &str) -> PathBuf {
    ["/usr/bin", "/bin"]
        .iter()
        .map(|d| Path::new(d).join(p))
        .find(|f| f.exists())
        .unwrap()
}

/// The release of tag NAME-v0.2.0 holds the program's file (a script that says its name) and its hash.
fn publish(l: &Lab, program: &str, hash: Option<&str>) {
    let tag = l.release.join(format!("{program}-v0.2.0"));
    fs::create_dir_all(&tag).unwrap();
    let body = format!("#!/bin/sh\necho {program} asset\n");
    let file = tag.join(format!("{program}-{SUFFIX}"));
    fs::write(&file, &body).unwrap();
    let real = {
        // the hash of what was written, by the same tool a person would check it with
        let o = Command::new("sha256sum").arg(&file).output().unwrap();
        String::from_utf8(o.stdout)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .to_string()
    };
    fs::write(
        tag.join(format!("{program}-{SUFFIX}.sha256")),
        format!("{}  {program}-{SUFFIX}\n", hash.unwrap_or(&real)),
    )
    .unwrap();
}

fn host(l: &Lab, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lpoco-host"))
        .args(args)
        .env_clear()
        .env("HOME", l.dir.join("home"))
        .env("PATH", l.dir.join("stubs"))
        .env("LPOCO_LAB", l.dir.join("lab.toml"))
        .env("LPOCO_ROOT", l.dir.join("stubs/root"))
        .env("LPOCO_FS_ROOT", l.dir.join("fs"))
        .env(
            "LPOCO_RELEASE_BASE",
            format!("file://{}", l.release.display()),
        )
        .output()
        .unwrap()
}

fn text(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn calls(l: &Lab) -> String {
    fs::read_to_string(&l.calls).unwrap_or_default()
}

fn installed(l: &Lab, p: &str) -> PathBuf {
    l.dir.join("home/.local/bin").join(p)
}

#[test]
fn the_plan_names_the_asset_and_its_hash_and_fetches_nothing() {
    let l = lab("plan");
    let o = host(&l, &["plan"]);
    let s = text(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains(&format!(
            "{}/lpoco-lab-v0.2.0/lpoco-lab-{SUFFIX}",
            format_args!("file://{}", l.release.display())
        )),
        "{s}"
    );
    assert!(
        s.contains(".sha256") && s.contains("else cargo install"),
        "{s}"
    );
    assert!(
        !calls(&l).contains("curl"),
        "the plan fetched: {}",
        calls(&l)
    );
    assert!(!l.dir.join("home/.local").exists());
}

#[test]
fn prepare_installs_each_program_from_its_asset_after_the_hash_matches_and_builds_nothing() {
    let l = lab("good");
    for p in FAMILY {
        publish(&l, p, None);
    }
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text(&o));
    for p in FAMILY {
        let f = installed(&l, p);
        assert!(f.exists(), "{p} not installed: {}", text(&o));
        assert_eq!(
            fs::metadata(&f).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(fs::read_to_string(&f)
            .unwrap()
            .contains(&format!("{p} asset")));
    }
    // the old names are second binaries of the same programs: the same file stands at each
    for (old, from) in [("devlab", "lpoco-lab"), ("tone", "lpoco-tone")] {
        assert_eq!(
            fs::read(installed(&l, old)).unwrap(),
            fs::read(installed(&l, from)).unwrap(),
            "{old}"
        );
    }
    assert!(!calls(&l).contains("cargo"), "cargo ran: {}", calls(&l));
}

#[test]
fn a_wrong_hash_is_refused_nothing_is_installed_and_cargo_is_not_the_way_round() {
    let l = lab("bad");
    for p in FAMILY {
        publish(&l, p, None);
    }
    publish(&l, "lpoco", Some(&"0".repeat(64)));
    let o = host(&l, &["prepare", "now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("lpoco") && s.contains("sha256"), "{s}");
    assert!(
        !installed(&l, "lpoco").exists(),
        "a file with the wrong hash was installed"
    );
    assert!(
        !calls(&l).contains("cargo"),
        "a wrong hash fell back to a build: {}",
        calls(&l)
    );
}

#[test]
fn a_release_without_the_asset_falls_back_to_cargo_for_that_program_only() {
    let l = lab("none");
    for p in FAMILY.iter().filter(|p| **p != "lpoco-tone") {
        publish(&l, p, None);
    }
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text(&o));
    let c = calls(&l);
    assert!(c.contains("--tag lpoco-tone-v0.2.0 lpoco-tone"), "{c}");
    assert_eq!(c.matches("cargo install").count(), 1, "{c}");
    assert!(installed(&l, "lpoco-lab").exists());
}

#[test]
fn the_workflow_that_attaches_assets_names_every_program_the_host_fetches() {
    let flow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../.github/workflows/assets.yml"),
    )
    .expect("the assets workflow");
    let attached: Vec<&str> = flow
        .lines()
        .find(|l| l.trim_start().starts_with("for p in "))
        .expect("a loop over the programs")
        .trim()
        .trim_start_matches("for p in ")
        .trim_end_matches("; do")
        .split_whitespace()
        .collect();
    let mut want: Vec<&str> = FAMILY.to_vec();
    want.sort();
    let mut have = attached.clone();
    have.sort();
    assert_eq!(have, want, "the workflow attaches {attached:?}");
    assert!(
        flow.contains(&format!("{SUFFIX}\n")) || flow.contains(SUFFIX),
        "target named"
    );
}

#[test]
fn a_fallback_build_forces_over_a_binary_an_earlier_asset_install_left() {
    let l = lab("force");
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text(&o));
    // cargo has no record of a file the host wrote itself: it refuses it unless told to replace
    assert!(
        calls(&l).contains("cargo install --locked --git") && calls(&l).contains("--force"),
        "{}",
        calls(&l)
    );
}

#[test]
fn an_asset_without_its_hash_or_with_a_garbled_one_is_refused_and_never_built() {
    for (name, hash_file) in [("nohash", None), ("garbled", Some("not a hash\n"))] {
        let l = lab(name);
        for p in FAMILY {
            publish(&l, p, None);
        }
        let sha = l
            .release
            .join(format!("lpoco-v0.2.0/lpoco-{SUFFIX}.sha256"));
        match hash_file {
            None => fs::remove_file(&sha).unwrap(),
            Some(t) => fs::write(&sha, t).unwrap(),
        }
        let o = host(&l, &["prepare", "now"]);
        assert!(!o.status.success(), "{name}: {}", text(&o));
        assert!(!installed(&l, "lpoco").exists(), "{name}");
        assert!(!calls(&l).contains("cargo"), "{name}: {}", calls(&l));
    }
}

#[test]
fn an_outage_is_a_failure_not_a_missing_asset() {
    let l = lab("outage");
    // a curl that answers 503
    exe(&l.dir.join("stubs/curl"), "printf 503; exit 22");
    let o = host(&l, &["prepare", "now"]);
    let s = text(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("503"), "{s}");
    assert!(
        !calls(&l).contains("cargo"),
        "an outage was built around: {}",
        calls(&l)
    );
}

#[test]
fn only_x86_64_linux_glibc_hosts_fit_an_asset() {
    assert!(!lpoco_host::asset_fits("linux", "aarch64"));
    assert!(!lpoco_host::asset_fits("macos", "x86_64"));
    assert_eq!(
        lpoco_host::asset_fits("linux", "x86_64"),
        cfg!(target_env = "gnu")
    );
}
