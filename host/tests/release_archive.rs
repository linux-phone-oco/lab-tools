// SPDX-License-Identifier: MIT OR Apache-2.0
//! A tool whose release ships a tar.gz and a checksums file, not a bare binary (backlog nfpm-on-the-lab-host):
//! `[tools.nfpm]` adds `checksums` (the release's file of `HASH  NAME` lines) and `member` (the file in the archive);
//! prepare fetches both, finds the archive's line, installs the member only when the archive's hash matches, stamps
//! the tag. The checksums file's shape is captured text of goreleaser/nfpm v2.49.0's checksums.txt: one line per
//! asset, `HASH  NAME`, two spaces (the file also lists `NAME.sbom.json`, kept below as the neighbour a lookup must skip).

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REPO: &str = "goreleaser/nfpm";
const TAG: &str = "v2.49.0";
const ASSET: &str = "nfpm_2.49.0_Linux_x86_64.tar.gz";
const SBOM_LINE: &str =
    "d47d9aeafd1790731cf7d0078429462895ee35f1c83d12ba97c349abc8c8f734  nfpm_2.49.0_Linux_x86_64.tar.gz.sbom.json";

struct Lab {
    dir: PathBuf,
    github: PathBuf,
    calls: PathBuf,
}

fn exe(path: &Path, body: &str) {
    fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn lab(name: &str, pin_extra: &str) -> Lab {
    let dir =
        std::env::temp_dir().join(format!("lpoco-host-archive-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let stubs = dir.join("stubs");
    let github = dir.join("github");
    fs::create_dir_all(&stubs).unwrap();
    fs::create_dir_all(dir.join("home")).unwrap();
    fs::create_dir_all(&github).unwrap();
    fs::write(
        dir.join("lab.toml"),
        format!(
            "devices = \"{}/none/*/device.toml\"\n[tools.nfpm]\nrepository = \"{REPO}\"\ntag = \"{TAG}\"\nasset = \"{ASSET}\"\n{pin_extra}",
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
    for p in ["cp", "tar", "gzip"] {
        let real = ["/usr/bin", "/bin"]
            .iter()
            .map(|d| Path::new(d).join(p))
            .find(|f| f.exists())
            .unwrap();
        std::os::unix::fs::symlink(real, stubs.join(p)).unwrap();
    }
    Lab { dir, github, calls }
}

const PIN: &str = "checksums = \"checksums.txt\"\nmember = \"nfpm\"\n";

/// The release holds the archive (a tar.gz of a script that says its name, next to a README the lookup must not install)
/// and the checksums file; `hash` when the archive's line is to be wrong.
fn publish(l: &Lab, hash: Option<&str>, member: &str) {
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    fs::create_dir_all(&folder).unwrap();
    let src = l.dir.join("src");
    fs::create_dir_all(&src).unwrap();
    fs::write(src.join(member), "#!/bin/sh\necho nfpm asset\n").unwrap();
    fs::write(src.join("README.md"), "not the tool\n").unwrap();
    let ok = Command::new("tar")
        .args(["-czf"])
        .arg(folder.join(ASSET))
        .arg("-C")
        .arg(&src)
        .arg(member)
        .arg("README.md")
        .status()
        .unwrap();
    assert!(ok.success());
    let o = Command::new("sha256sum")
        .arg(folder.join(ASSET))
        .output()
        .unwrap();
    let real = String::from_utf8(o.stdout).unwrap();
    let real = real.split_whitespace().next().unwrap();
    fs::write(
        folder.join("checksums.txt"),
        format!("{}  {ASSET}\n{SBOM_LINE}\n", hash.unwrap_or(real)),
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
    l.dir.join("home/.local/bin/nfpm")
}

fn stamp(l: &Lab) -> PathBuf {
    l.dir.join("home/.local/share/lpoco-host/rev-nfpm")
}

#[test]
fn the_plan_names_the_archive_the_checksums_file_and_the_member_and_fetches_nothing() {
    let l = lab("plan", PIN);
    let o = host(&l, &["plan"]);
    let s = text_of(&o);
    assert!(o.status.success(), "{s}");
    assert!(
        s.contains(&format!("{REPO}/releases/download/{TAG}/{ASSET}")),
        "{s}"
    );
    assert!(s.contains("checksums.txt") && s.contains("nfpm"), "{s}");
    assert!(
        !calls(&l).contains("curl"),
        "the plan fetched: {}",
        calls(&l)
    );
    assert!(!installed(&l).exists());
}

#[test]
fn prepare_installs_the_member_when_the_archives_hash_matches_and_stamps_the_tag() {
    let l = lab("good", PIN);
    publish(&l, None, "nfpm");
    let o = host(&l, &["prepare", "now"]);
    assert!(o.status.success(), "{}", text_of(&o));
    let f = installed(&l);
    assert_eq!(
        fs::metadata(&f).unwrap().permissions().mode() & 0o777,
        0o755
    );
    assert!(fs::read_to_string(&f).unwrap().contains("nfpm asset"));
    assert!(!l.dir.join("home/.local/bin/README.md").exists());
    assert_eq!(fs::read_to_string(stamp(&l)).unwrap().trim(), TAG);
    assert!(!calls(&l).contains("cargo"), "cargo ran: {}", calls(&l));
}

#[test]
fn a_wrong_hash_for_the_archive_installs_nothing_and_stamps_nothing() {
    let l = lab("bad", PIN);
    publish(&l, Some(&"0".repeat(64)), "nfpm");
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success(), "{s}");
    assert!(s.contains("sha256") && s.contains("not installed"), "{s}");
    assert!(!installed(&l).exists());
    assert!(!stamp(&l).exists());
}

#[test]
fn a_checksums_file_without_the_archives_line_installs_nothing() {
    let l = lab("no-line", PIN);
    publish(&l, None, "nfpm");
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    fs::write(folder.join("checksums.txt"), format!("{SBOM_LINE}\n")).unwrap();
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(
        !o.status.success() && s.contains(ASSET) && s.contains("not installed"),
        "{s}"
    );
    assert!(!installed(&l).exists());
}

#[test]
fn an_archive_without_the_member_installs_nothing() {
    let l = lab("no-member", PIN);
    publish(&l, None, "other");
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(
        !o.status.success() && s.contains("nfpm") && s.contains("not installed"),
        "{s}"
    );
    assert!(!installed(&l).exists());
    assert!(!stamp(&l).exists());
}

#[test]
fn a_member_that_could_leave_the_install_folder_or_a_checksums_name_that_is_not_one_word_is_refused(
) {
    for (i, (extra, word)) in [
        (
            "checksums = \"checksums.txt\"\nmember = \"../x\"\n",
            "member",
        ),
        (
            "checksums = \"checksums.txt\"\nmember = \"a/b\"\n",
            "member",
        ),
        ("checksums = \"a/b\"\nmember = \"nfpm\"\n", "checksums"),
        ("member = \"nfpm\"\n", "checksums"),
        ("checksums = \"checksums.txt\"\n", "member"),
    ]
    .iter()
    .enumerate()
    {
        let l = lab(&format!("bad-pin-{i}"), extra);
        let o = host(&l, &["plan"]);
        let s = text_of(&o);
        assert!(!o.status.success() && s.contains(word), "{extra}: {s}");
    }
}

#[test]
fn a_member_that_is_a_symlink_installs_nothing() {
    let l = lab("symlink", PIN);
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    fs::create_dir_all(&folder).unwrap();
    let src = l.dir.join("src");
    fs::create_dir_all(&src).unwrap();
    std::os::unix::fs::symlink("/etc/hostname", src.join("nfpm")).unwrap();
    assert!(Command::new("tar")
        .arg("-czf")
        .arg(folder.join(ASSET))
        .arg("-C")
        .arg(&src)
        .arg("nfpm")
        .status()
        .unwrap()
        .success());
    let o = Command::new("sha256sum")
        .arg(folder.join(ASSET))
        .output()
        .unwrap();
    let hash = String::from_utf8(o.stdout).unwrap();
    fs::write(
        folder.join("checksums.txt"),
        format!("{}  {ASSET}\n", hash.split_whitespace().next().unwrap()),
    )
    .unwrap();
    let o = host(&l, &["prepare", "now"]);
    let s = text_of(&o);
    assert!(!o.status.success() && s.contains("not installed"), "{s}");
    assert!(!installed(&l).exists());
}

#[test]
fn the_first_line_for_the_archive_decides_even_when_its_hash_is_broken() {
    let l = lab("first-line", PIN);
    publish(&l, None, "nfpm");
    let folder = l.github.join(REPO).join("releases/download").join(TAG);
    let good = fs::read_to_string(folder.join("checksums.txt")).unwrap();
    fs::write(folder.join("checksums.txt"), format!("zz  {ASSET}\n{good}")).unwrap();
    let o = host(&l, &["prepare", "now"]);
    assert!(!o.status.success(), "{}", text_of(&o));
    assert!(!installed(&l).exists());
}
