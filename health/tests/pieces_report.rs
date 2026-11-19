//! The suite check lab-pieces (backlog health-reports-pieces): the phone reports what lab pieces it holds -
//! each declared file's git blob id and each lab-tools package's version, one row apiece, reading only.
//! Run here against a made-up phone under LPOCO_HEALTH_ROOT with a made-up `apk`; the blob id is the one
//! `git hash-object` gives for the same file. `lpoco host check` compares these rows with what the lab
//! pins (backlog phone-pieces-drift-named).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHECK: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/suite/checks/base/94-lab-pieces.sh"
);
const LIB: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/suite/lib.sh");

fn phone(name: &str, env: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("pieces-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("etc/lpoco-health")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("etc/lpoco-health/health.env"), env).unwrap();
    // an apk that says what the phone's database says
    let apk = root.join("bin/apk");
    fs::write(
        &apk,
        "#!/bin/sh\n[ \"$1 $2\" = 'list --installed' ] || exit 2\n\
         echo 'musl-1.2.5-r9 aarch64 {musl} (MIT) [installed]'\n\
         echo 'lpoco-health-0.12.1-r0 aarch64 {lpoco-health} (MIT OR Apache-2.0) [installed]'\n",
    )
    .unwrap();
    assert!(Command::new("chmod")
        .arg("755")
        .arg(&apk)
        .status()
        .unwrap()
        .success());
    root
}

fn run(root: &Path) -> String {
    let o = Command::new("timeout")
        .args(["20", "sh", CHECK])
        .env("LPOCO_HEALTH_ROOT", root)
        .env("LPOCO_HEALTH_LIB", LIB)
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin", root.join("bin").display()),
        )
        .output()
        .unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn git_blob(file: &Path) -> String {
    let o = Command::new("git")
        .arg("hash-object")
        .arg(file)
        .output()
        .unwrap();
    String::from_utf8_lossy(&o.stdout).trim().to_string()
}

#[test]
fn a_declared_file_and_a_package_each_report_a_row() {
    let root = phone(
        "both",
        "PIECE_FILES='/etc/lpoco/awake.conf'\nPIECE_PACKAGES='lpoco-health'\n",
    );
    fs::create_dir_all(root.join("etc/lpoco")).unwrap();
    // a body with a NUL and no final newline: the blob id is of the bytes, not of the lines
    fs::write(root.join("etc/lpoco/awake.conf"), b"gate=one\0two").unwrap();
    let out = run(&root);
    let blob = git_blob(&root.join("etc/lpoco/awake.conf"));
    assert_eq!(blob.len(), 40, "git hash-object gives a blob id");
    let rows: Vec<&str> = out.lines().collect();
    assert_eq!(rows.len(), 2, "one row per file and per package: {out}");
    assert!(
        rows.iter().any(
            |r| r.starts_with("pass piece-file:/etc/lpoco/awake.conf lab-pieces ")
                && r.ends_with(&format!("/etc/lpoco/awake.conf {blob}"))
        ),
        "{out}"
    );
    assert!(
        rows.iter().any(
            |r| r.starts_with("pass piece-package-lpoco-health lab-pieces ")
                && r.ends_with("lpoco-health 0.12.1-r0")
        ),
        "{out}"
    );
}

#[test]
fn a_file_or_package_the_phone_lacks_is_named_missing() {
    let root = phone(
        "lacks",
        "PIECE_FILES='/etc/nowhere'\nPIECE_PACKAGES='lpoco-ghost'\n",
    );
    let out = run(&root);
    assert!(out.contains("/etc/nowhere missing"), "{out}");
    assert!(out.contains("lpoco-ghost missing"), "{out}");
}

#[test]
fn unsaid_the_lab_tools_package_is_the_one_reported() {
    let root = phone("plain", "DEVICE='p'\n");
    let out = run(&root);
    assert!(out.contains("lpoco-health 0.12.1-r0"), "{out}");
    assert_eq!(out.lines().count(), 1, "{out}");
}

#[test]
fn a_declared_user_config_reports_its_blob_and_its_package_defaults_and_an_undeclared_one_is_not_read(
) {
    let root = phone(
        "config",
        "PIECE_CONFIGS='/home/u/hook.sh=/usr/share/x/hook.sh /home/u/profile /home/u/gone=/usr/share/x/gone'\n",
    );
    fs::create_dir_all(root.join("home/u")).unwrap();
    fs::create_dir_all(root.join("usr/share/x")).unwrap();
    fs::write(root.join("home/u/hook.sh"), b"edited\n").unwrap();
    fs::write(root.join("usr/share/x/hook.sh"), b"shipped\n").unwrap();
    fs::write(root.join("home/u/profile"), b"export X=1\n").unwrap();
    fs::write(root.join("home/u/undeclared"), b"never read\n").unwrap();
    let out = run(&root);
    let b = |p: &str| git_blob(&root.join(p));
    assert!(
        out.contains(&format!(
            "/home/u/hook.sh {} {}",
            b("home/u/hook.sh"),
            b("usr/share/x/hook.sh")
        )),
        "{out}"
    );
    assert!(
        out.contains(&format!("/home/u/profile {} -", b("home/u/profile"))),
        "against the base's copy there is no default blob: {out}"
    );
    assert!(out.contains("/home/u/gone missing missing"), "{out}");
    assert!(!out.contains("undeclared"), "{out}");
}
