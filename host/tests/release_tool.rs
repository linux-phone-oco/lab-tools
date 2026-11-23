// SPDX-License-Identifier: MIT OR Apache-2.0
//! A tool of another project is installed from its release at its pin (backlog host-installs-cargo-tool-at-pin):
//! `[tools.rhizoid]` names a GitHub repository, a tag and an asset; prepare fetches the asset and its `.sha256`,
//! installs it only when the hash matches, stamps the tag, and never builds with cargo. The release is a folder
//! (`LPOCO_GITHUB=file://DIR`, then DIR/REPOSITORY/releases/download/TAG/ASSET), curl a stub that copies from it.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REPO: &str = "linux-phone-oco/rhizoid";
const TAG: &str = "v0.2.2";
const ASSET: &str = "rhizoid-x86_64-unknown-linux-gnu";

struct Lab {
    dir: PathBuf,
    github: PathBuf,
    calls: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn lab(name: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("lpoco-host-tool-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let stubs = dir.join("stubs");
    let github = dir.join("github");
    fs::create_dir_all(&stubs).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(&github).unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/none/*/device.toml\"\n[tools.rhizoid]\nrepository = \"{REPO}\"\ntag = \"{TAG}\"\nasset = \"{ASSET}\"\n",
            dir.display()
        ),
    )
    .unwrap();
    let calls = dir.join("calls");
    let rec = format!("echo \"$0 $@\" >> {}", calls.display());
    for s in ["root", "cargo", "git", "systemctl"] {
        exe(&stubs.join(s), &rec);
    }
    exe(
        &stubs.join("curl"),
        &format!(
            "echo \"$0 $@\" >> {}\nwhile [ $# -gt 0 ]; do case \"$1\" in --output) out=\"$2\"; shift;; --*) ;; *) url=\"$1\";; esac; shift; done\nsrc=\"${{url#file://}}\"\n[ -f \"$src\" ] || {{ printf 404; exit 22; }}\nprintf 200\ncp \"$src\" \"$out\"",
            calls.display()
        ),
    );
    std::os::unix::fs::symlink("/usr/bin/cp", stubs.join("cp")).unwrap();
    Lab { dir, github, calls }
}

/// The release holds the program's file (a script that says its name) and its hash, `hash` when it is to be wrong.
fn publish(l: &Lab, hash: Option<&str>) {
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    fs::create_dir_all(&folder).unwrap();
    let file = folder.join(ASSET);
    fs::write(&file, "#!/bin/sh\necho rhizoid asset\n").unwrap();
    let o = Command::new("sha256sum").arg(&file).output().unwrap();
    let real = String::from_utf8(o.stdout).unwrap();
    let real = real.split_whitespace().next().unwrap();
    fs::write(
        folder.join(format!("{ASSET}.sha256")),
        format!("{}  {ASSET}\n", hash.unwrap_or(real)),
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
        .env("LPOCO_GITHUB", format!("file://{}", l.github.display()))
        .output()
        .unwrap()
}

fn text_of(o: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    )
}

fn calls(l: &Lab) -> String {
    fs::read_to_string(&l.calls).unwrap_or_default()
}

fn installed(l: &Lab) -> PathBuf {
    l.dir.join("home/.local/bin/rhizoid")
}

fn stamp(l: &Lab) -> PathBuf {
    l.dir.join("home/.local/share/lpoco-host/rev-rhizoid")
}

#[test]
fn the_plan_names_the_asset_and_its_hash_and_fetches_nothing() {
    let l = lab("plan");
    let o = host(&l, &["plan"]);
    let s = text_of(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains(&format!("{REPO}/releases/download/{TAG}/{ASSET}")),
        "{s}"
    );
    assert!(s.contains(".sha256"), "{s}");
    assert!(
        !calls(&l).contains("curl"),
        "the plan fetched: {}",
        calls(&l)
    );
    assert!(!installed(&l).exists());
}

#[test]
fn prepare_installs_the_asset_when_its_hash_matches_and_stamps_the_tag() {
    let l = lab("good");
    publish(&l, None);
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text_of(&o));
    let f = installed(&l);
    assert_eq!(
        fs::metadata(&f).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(fs::read_to_string(&f).unwrap().contains("rhizoid asset"));
    assert_eq!(fs::read_to_string(stamp(&l)).unwrap().trim(), TAG);
    assert!(!calls(&l).contains("cargo"), "cargo ran: {}", calls(&l));
}

#[test]
fn a_tool_at_its_pin_is_left_alone() {
    let l = lab("at-pin");
    publish(&l, None);
    assert!(host(&l, &["prepare", "now"]).status.success());
    let _ = fs::remove_file(&l.calls);
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text_of(&o));
    assert!(!calls(&l).contains("curl"), "fetched again: {}", calls(&l));
}

#[test]
fn a_wrong_hash_is_refused_and_nothing_is_installed_or_stamped() {
    let l = lab("bad");
    publish(&l, Some(&"0".repeat(64)));
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("sha256") && s.contains("not installed"), "{s}");
    assert!(!installed(&l).exists());
    assert!(!stamp(&l).exists());
    assert!(!calls(&l).contains("cargo"), "cargo ran: {}", calls(&l));
}

#[test]
fn a_release_without_the_asset_installs_nothing_and_does_not_build() {
    let l = lab("none");
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains(ASSET), "{s}");
    assert!(!installed(&l).exists());
    assert!(!calls(&l).contains("cargo"), "cargo ran: {}", calls(&l));
}

#[test]
fn a_pin_that_could_leave_the_release_url_or_replace_a_family_program_is_refused() {
    let bad = [
        ("repository = \"a\"", "repository"),
        ("repository = \"a/b/c\"", "repository"),
        ("repository = \"a/..\"", "repository"),
        ("repository = \"a/b?x\"", "repository"),
        ("tag = \"../../x/y/raw/main\"", "tag"),
        ("tag = \"-v\"", "tag"),
        ("tag = \"\"", "tag"),
        ("asset = \"x/y\"", "asset"),
        ("asset = \"x#y\"", "asset"),
    ];
    for (i, (line, word)) in bad.iter().enumerate() {
        let l = lab(&format!("bad-pin-{i}"));
        let key = line.split(' ').next().unwrap();
        let text = fs::read_to_string(l.dir.join("lab.toml")).unwrap();
        let text: String = text
            .lines()
            .map(|t| if t.starts_with(key) { *line } else { t })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(l.dir.join("lab.toml"), text).unwrap();
        let o = host(&l, &["plan"]);
        let s = text_of(&o);
        assert!(!o.status.success() && s.contains(word), "{line}: {s}");
    }
    let l = lab("family-name");
    let t = fs::read_to_string(l.dir.join("lab.toml")).unwrap();
    fs::write(
        l.dir.join("lab.toml"),
        t.replace("[tools.rhizoid]", "[tools.lpoco-host]"),
    )
    .unwrap();
    let o = host(&l, &["plan"]);
    assert!(
        !o.status.success() && text_of(&o).contains("family"),
        "{}",
        text_of(&o)
    );
}

#[test]
fn a_release_without_the_hash_installs_nothing() {
    let l = lab("no-hash");
    publish(&l, None);
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    fs::remove_file(folder.join(format!("{ASSET}.sha256"))).unwrap();
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success() && s.contains("no .sha256"), "{s}");
    assert!(!installed(&l).exists());
}
