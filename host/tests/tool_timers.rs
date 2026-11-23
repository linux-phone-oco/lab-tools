// SPDX-License-Identifier: MIT OR Apache-2.0
//! A tool that ships user timers gets them from lab.toml (backlog host-declares-tool-timers): `[tools.rhizoid]
//! timers = ["rhizoid-bump"]` writes the release tag's `packaging/systemd/NAME.service|.timer` as user units, its
//! `env_file` (mode 600) from `env`, and enables the timer; plan names what differs, a hand-changed unit included.
//! The release is a folder (`LPOCO_GITHUB=file://DIR`, then DIR/REPOSITORY/raw/TAG/packaging/systemd/FILE).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REPO: &str = "linux-phone-oco/rhizoid";
const TAG: &str = "v0.3.6";
const ASSET: &str = "rhizoid-x86_64-unknown-linux-gnu";
const SERVICE: &str = "[Unit]\nDescription=bump\n[Service]\nType=oneshot\nEnvironmentFile=%h/.config/rhizoid/bump.env\nExecStart=/bin/true\n";
const TIMER: &str = "[Unit]\nDescription=bump hourly\n[Timer]\nOnCalendar=hourly\n[Install]\nWantedBy=timers.target\n";

struct Lab {
    dir: PathBuf,
    github: PathBuf,
    calls: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn lab(name: &str, declared: &str) -> Lab {
    let dir = std::env::temp_dir().join(format!("lpoco-host-timers-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let stubs = dir.join("stubs");
    let github = dir.join("github");
    fs::create_dir_all(&stubs).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(&github).unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/none/*/device.toml\"\n[tools.rhizoid]\nrepository = \"{REPO}\"\ntag = \"{TAG}\"\nasset = \"{ASSET}\"\n{declared}",
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

const DECLARED: &str = "timers = [\"rhizoid-bump\"]\nenv_file = \".config/rhizoid/bump.env\"\nenv = { RHIZOID_GRAPH_CLONE = \"~/Projects/linux-phone-oco/docs\" }\n";

/// The tag's packaging folder holds the units, `service` / `timer` when they are to differ.
fn publish(l: &Lab, service: Option<&str>, timer: Option<&str>) {
    let rel = l.github.join(REPO).join("releases/download").join(TAG);
    fs::create_dir_all(&rel).unwrap();
    fs::write(rel.join(ASSET), "#!/bin/sh\necho rhizoid asset\n").unwrap();
    let sum = Command::new("sha256sum")
        .arg(rel.join(ASSET))
        .output()
        .unwrap();
    let sum = String::from_utf8(sum.stdout).unwrap();
    let hash = sum.split_whitespace().next().unwrap();
    fs::write(
        rel.join(format!("{ASSET}.sha256")),
        format!("{hash}  {ASSET}\n"),
    )
    .unwrap();
    let folder = l
        .github
        .join(REPO)
        .join("raw")
        .join(TAG)
        .join("packaging/systemd");
    fs::create_dir_all(&folder).unwrap();
    if let Some(s) = service {
        fs::write(folder.join("rhizoid-bump.service"), s).unwrap();
    }
    if let Some(t) = timer {
        fs::write(folder.join("rhizoid-bump.timer"), t).unwrap();
    }
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

fn unit(l: &Lab, name: &str) -> PathBuf {
    l.dir.join("home/.config/systemd/user").join(name)
}

fn env_file(l: &Lab) -> PathBuf {
    l.dir.join("home/.config/rhizoid/bump.env")
}

#[test]
fn prepare_writes_the_tags_units_and_a_private_env_file_and_enables_the_timer() {
    let l = lab("write", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(o.status.success(), "{s}");
    assert_eq!(
        fs::read_to_string(unit(&l, "rhizoid-bump.service")).unwrap(),
        SERVICE
    );
    assert_eq!(
        fs::read_to_string(unit(&l, "rhizoid-bump.timer")).unwrap(),
        TIMER
    );
    assert_eq!(
        fs::read_to_string(env_file(&l)).unwrap(),
        format!(
            "RHIZOID_GRAPH_CLONE={}/home/Projects/linux-phone-oco/docs\n",
            l.dir.display()
        )
    );
    let mode = fs::metadata(env_file(&l)).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600, "the env file is private");
    let calls = fs::read_to_string(&l.calls).unwrap();
    assert!(calls.contains("--user daemon-reload"), "{calls}");
    assert!(
        calls.contains("--user enable --now rhizoid-bump.timer"),
        "{calls}"
    );
}

#[test]
fn plan_names_the_units_changes_nothing_and_a_prepared_host_is_as_declared() {
    let l = lab("plan", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    let o = host(&l, &["plan"]);
    let s = text_of(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains("rhizoid-bump.timer (new)") && s.contains("bump.env (new)"),
        "{s}"
    );
    assert!(!unit(&l, "rhizoid-bump.timer").exists());
    assert!(host(&l, &["prepare", "now"]).status.success());
    let s = text_of(&host(&l, &["plan"]));
    assert!(s.contains("rhizoid-bump.timer (as declared)"), "{s}");
    assert!(s.contains("bump.env (as declared)"), "{s}");
}

#[test]
fn a_hand_changed_unit_is_reported_and_prepare_puts_the_tags_text_back() {
    let l = lab("drift", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    assert!(host(&l, &["prepare", "now"]).status.success());
    fs::write(
        unit(&l, "rhizoid-bump.timer"),
        TIMER.replace("hourly", "minutely"),
    )
    .unwrap();
    let s = text_of(&host(&l, &["plan"]));
    assert!(s.contains("rhizoid-bump.timer (differs"), "{s}");
    assert!(host(&l, &["prepare", "now"]).status.success());
    assert_eq!(
        fs::read_to_string(unit(&l, "rhizoid-bump.timer")).unwrap(),
        TIMER
    );
}

#[test]
fn a_tag_without_the_unit_writes_nothing_and_names_what_is_missing() {
    let l = lab("missing", DECLARED);
    publish(&l, Some(SERVICE), None);
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("packaging/systemd/rhizoid-bump.timer"), "{s}");
    assert!(!unit(&l, "rhizoid-bump.service").exists());
    assert!(!env_file(&l).exists());
}

#[test]
fn a_timer_declaration_that_could_leave_its_folders_is_refused() {
    let bad = [
        "timers = [\"../x\"]\n",
        "timers = [\"a b\"]\n",
        "timers = [\"x\"]\nenv_file = \"../x.env\"\n",
        "timers = [\"x\"]\nenv_file = \"/etc/x.env\"\n",
        "timers = [\"x\"]\nenv = { A = \"1\" }\n",
        "timers = [\"x\"]\nenv_file = \"x.env\"\nenv = { \"a b\" = \"1\" }\n",
        "timers = [\"x\"]\nenv_file = \"x.env\"\nenv = { A = \"1\\n2\" }\n",
        "timers = [\"x\"]\nenv_file = \"x.env\"\nenv = { A = \"a\\\\\" }\n",
        "timers = [\"x\"]\nenv_file = \"x.env\"\nenv = { A = \"a\\\"b\" }\n",
    ];
    for (i, d) in bad.iter().enumerate() {
        let l = lab(&format!("bad-{i}"), d);
        let o = host(&l, &["plan"]);
        assert!(!o.status.success(), "{d}: {}", text_of(&o));
        assert!(
            text_of(&o).contains("[tools.rhizoid]"),
            "{d}: {}",
            text_of(&o)
        );
    }
}

#[test]
fn plan_never_prints_the_env_files_values() {
    let l = lab("secret", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    let s = text_of(&host(&l, &["plan"]));
    assert!(
        s.contains("bump.env (new)") && !s.contains("RHIZOID_GRAPH_CLONE="),
        "{s}"
    );
    assert!(host(&l, &["prepare", "now"]).status.success());
    fs::write(env_file(&l), "GITHUB_TOKEN=ghp_SECRET\n").unwrap();
    let s = text_of(&host(&l, &["plan"]));
    assert!(
        s.contains("bump.env (differs") && !s.contains("ghp_SECRET"),
        "{s}"
    );
}

#[test]
fn a_loose_env_file_mode_is_named_and_set_back() {
    let l = lab("mode", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    assert!(host(&l, &["prepare", "now"]).status.success());
    fs::set_permissions(env_file(&l), fs::Permissions::from_mode(0o644)).unwrap();
    let s = text_of(&host(&l, &["plan"]));
    assert!(s.contains("bump.env (mode differs)"), "{s}");
    assert!(host(&l, &["prepare", "now"]).status.success());
    assert_eq!(
        fs::metadata(env_file(&l)).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn a_changed_timer_that_was_installed_is_restarted_and_a_new_one_is_not() {
    let l = lab("again", DECLARED);
    publish(&l, Some(SERVICE), Some(TIMER));
    assert!(host(&l, &["prepare", "now"]).status.success());
    assert!(!fs::read_to_string(&l.calls).unwrap().contains("restart"));
    publish(&l, Some(SERVICE), Some(&TIMER.replace("hourly", "daily")));
    assert!(host(&l, &["prepare", "now"]).status.success());
    let calls = fs::read_to_string(&l.calls).unwrap();
    assert!(
        calls.contains("--user restart rhizoid-bump.timer"),
        "{calls}"
    );
}
