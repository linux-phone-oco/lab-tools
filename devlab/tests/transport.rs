// SPDX-License-Identifier: MIT OR Apache-2.0
//! The phone is reached by the transport its device.toml declares (decision
//! 2026-11-19-phone-reached-by-its-declared-transport): telnet to its shell with no password, adb by its serial,
//! ssh as before; the suite says "not applicable" where there is no login, and asks for no password.

use std::io::Write;
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

struct Scratch(PathBuf);
impl Scratch {
    fn new(name: &str) -> Scratch {
        let d = std::env::temp_dir().join(format!("lpoco-transport-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("home")).unwrap();
        std::fs::create_dir_all(d.join("bin")).unwrap();
        Scratch(d)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A phone's shell on a TCP port: a real `sh` per connection, as a passwordless telnetd would start.
fn serve_a_shell() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let s = s.unwrap();
            let mut sh = Command::new("sh")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let (mut to_sh, mut from_sh) = (sh.stdin.take().unwrap(), sh.stdout.take().unwrap());
            let mut from_client = s.try_clone().unwrap();
            let mut to_client = s;
            std::thread::spawn(move || {
                let _ = pump(&mut from_client, &mut to_sh);
            });
            std::thread::spawn(move || {
                let _ = pump(&mut from_sh, &mut to_client);
                let _ = sh.wait();
                let _ = to_client.shutdown(std::net::Shutdown::Both);
            });
        }
    });
    port
}

fn run(s: &Scratch, manifest: &str, args: &[&str], stdin: &str) -> (i32, String, String) {
    let f = s.0.join("device.toml");
    std::fs::write(&f, manifest).unwrap();
    let path = format!(
        "{}:{}",
        s.0.join("bin").display(),
        std::env::var("PATH").unwrap()
    );
    let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
        .arg("--device")
        .arg(&f)
        .args(args)
        .current_dir(&s.0)
        .env("HOME", s.0.join("home"))
        .env("PATH", path)
        .env_remove("PHONE_PASS")
        .env_remove("XDG_STATE_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    // the binary may refuse and exit before it reads its input: a closed pipe is then its answer, not a failure
    let _ = c.stdin.take().unwrap().write_all(stdin.as_bytes());
    let o = c.wait_with_output().unwrap();
    (
        o.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&o.stdout).into_owned(),
        String::from_utf8_lossy(&o.stderr).into_owned(),
    )
}

fn telnet_device(port: u16) -> String {
    format!("[device]\ncodename = \"t\"\n[connection]\ntransport = \"telnet\"\nlogin = \"none\"\nuser = \"root\"\naddresses = [\"127.0.0.1\"]\nshell_address = \"127.0.0.1\"\nshell_port = {port}\n")
}

fn stand_in(dir: &Path, name: &str, body: &str) {
    use std::os::unix::fs::PermissionsExt;
    let f = dir.join("bin").join(name);
    std::fs::write(&f, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn a_telnet_phone_runs_the_script_at_its_shell_address_and_port_with_no_password() {
    let s = Scratch::new("telnet");
    let port = serve_a_shell();
    let (code, out, err) = run(
        &s,
        &telnet_device(port),
        &["phone", "sh"],
        "echo hello\necho there\n",
    );
    assert_eq!((code, out.as_str()), (0, "hello\nthere\n"), "{err}");
    let (code, out, _) = run(
        &s,
        &telnet_device(port),
        &["phone", "sh"],
        "echo before\nfalse\n",
    );
    assert_eq!(
        (code, out.as_str()),
        (1, "before\n"),
        "the phone's failure is the command's"
    );
    let (code, out, err) = run(&s, &telnet_device(port), &["phone", "root"], "echo root\n");
    assert_eq!(
        (code, out.as_str()),
        (0, "root\n"),
        "no sudo, no password on a shell with no login: {err}"
    );
}

#[test]
fn the_suite_says_not_applicable_where_the_device_has_no_login_and_asks_no_password() {
    let s = Scratch::new("suite");
    let port = serve_a_shell();
    let (code, out, err) = run(&s, &telnet_device(port), &["suite", "latest"], "");
    assert_eq!(code, 0, "{err}");
    assert!(
        out.contains("not applicable: device.toml declares no login"),
        "{out}"
    );
    assert!(!err.contains("password"), "no password was asked: {err}");
    let (code, out, _) = run(&s, &telnet_device(port), &["suite", "run"], "");
    assert_ne!(code, 0, "a suite that did not run is not a pass");
    assert!(out.contains("not applicable"), "{out}");
}

#[test]
fn an_adb_phone_runs_through_adb_dash_s_its_serial_shell() {
    let s = Scratch::new("adb");
    let calls = s.0.join("calls");
    stand_in(
        &s.0,
        "adb",
        &format!(
            "echo \"$*\" >> {}\n[ \"$3\" = shell ] && [ \"$4\" = true ] && exit 0\nexec sh -s",
            calls.display()
        ),
    );
    let manifest = "[device]\ncodename = \"a\"\n[connection]\ntransport = \"adb\"\nuser = \"root\"\naddresses = []\n[fastboot]\nserial = \"SER1\"\n";
    let (code, out, err) = run(&s, manifest, &["phone", "sh"], "echo via-adb\n");
    assert_eq!((code, out.as_str()), (0, "via-adb\n"), "{err}");
    let said = std::fs::read_to_string(&calls).unwrap();
    assert!(
        said.lines().all(|l| l.starts_with("-s SER1 shell")),
        "{said}"
    );
}

#[test]
fn an_adb_phone_with_no_serial_anywhere_is_refused_in_words() {
    let s = Scratch::new("adb-noserial");
    let manifest = "[device]\ncodename = \"a\"\n[connection]\ntransport = \"adb\"\nuser = \"root\"\naddresses = []\n";
    let (code, _, err) = run(&s, manifest, &["phone", "sh"], "true\n");
    assert_ne!(code, 0);
    assert!(err.contains("serial"), "{err}");
}

/// Bytes from one end to the other as they come (read and write, never the kernel's splice, which waits for more).
fn pump(from: &mut impl std::io::Read, to: &mut impl Write) -> std::io::Result<()> {
    let mut buf = [0u8; 4096];
    loop {
        let n = from.read(&mut buf)?;
        if n == 0 {
            return Ok(());
        }
        to.write_all(&buf[..n])?;
        to.flush()?;
    }
}

#[test]
fn an_ssh_phone_is_reached_as_before_through_sshpass_with_its_password() {
    let s = Scratch::new("ssh");
    let calls = s.0.join("calls");
    stand_in(
        &s.0,
        "sshpass",
        &format!(
            "echo \"$SSHPASS $*\" >> {}\ncase \"$*\" in *' true') exit 0;; esac\nexec sh -s",
            calls.display()
        ),
    );
    let manifest =
        "[device]\ncodename = \"s\"\n[connection]\nuser = \"me\"\naddresses = [\"10.9.9.9\"]\n";
    let with_pass = |script: &str| {
        std::fs::write(s.0.join("device.toml"), manifest).unwrap();
        let mut c = Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
            .args(["--device"])
            .arg(s.0.join("device.toml"))
            .args(["phone", "sh"])
            .env("HOME", s.0.join("home"))
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    s.0.join("bin").display(),
                    std::env::var("PATH").unwrap()
                ),
            )
            .env("PHONE_PASS", "pw1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        c.stdin
            .take()
            .unwrap()
            .write_all(script.as_bytes())
            .unwrap();
        c.wait_with_output().unwrap()
    };
    let o = with_pass("echo over-ssh\n");
    assert_eq!(
        String::from_utf8_lossy(&o.stdout),
        "over-ssh\n",
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let said = std::fs::read_to_string(&calls).unwrap();
    assert!(
        said.lines()
            .all(|l| l.starts_with("pw1 -e ssh") && l.contains("me@10.9.9.9")),
        "{said}"
    );
}

#[test]
fn a_script_that_ends_the_telnet_shell_is_an_error_saying_so_never_a_pass() {
    let s = Scratch::new("exit");
    let port = serve_a_shell();
    let (code, out, err) = run(
        &s,
        &telnet_device(port),
        &["phone", "sh"],
        "echo partial\nexit 0\n",
    );
    assert_ne!(code, 0, "{out}");
    assert!(
        err.contains("end marker") && err.contains("partial"),
        "{err}"
    );
}

#[test]
fn test_and_health_say_not_applicable_where_the_device_has_no_login_and_are_not_a_pass() {
    let s = Scratch::new("test-health");
    let port = serve_a_shell();
    for args in [&["test"][..], &["health"][..]] {
        let (code, out, err) = run(&s, &telnet_device(port), args, "");
        assert_eq!(
            code, 2,
            "{args:?}: a suite that did not run is not a pass: {err}"
        );
        assert!(
            out.contains("not applicable: device.toml declares no login"),
            "{args:?}: {out}"
        );
        assert!(
            !err.contains("password"),
            "{args:?}: no password was asked: {err}"
        );
    }
}

#[test]
fn the_watch_of_a_device_with_no_login_says_so_and_ends_instead_of_asking_after_a_suite_every_minute(
) {
    let s = Scratch::new("watch");
    let port = serve_a_shell();
    let (code, out, err) = run(&s, &telnet_device(port), &["watch"], "");
    assert_eq!(code, 0, "nothing to watch is not a failure: {err}");
    assert!(
        out.contains("not applicable: device.toml declares no login"),
        "{out}"
    );
    assert!(!out.contains("suite is not on the phone"), "{out}");
}

fn key_device(identity: &Path) -> String {
    key_device_as(identity, "root")
}

fn key_device_as(identity: &Path, user: &str) -> String {
    format!("[device]\ncodename = \"k\"\n[connection]\nlogin = \"key\"\nidentity = \"{}\"\nuser = \"{user}\"\naddresses = [\"10.9.9.8\"]\n", identity.display())
}

fn write_key(s: &Scratch, mode: u32) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let k = s.0.join("id_test");
    std::fs::write(&k, "not a real key\n").unwrap();
    std::fs::set_permissions(&k, std::fs::Permissions::from_mode(mode)).unwrap();
    k
}

#[test]
fn a_key_phone_is_reached_by_ssh_dash_i_its_identity_with_no_password_asked() {
    let s = Scratch::new("key");
    let calls = s.0.join("calls");
    stand_in(
        &s.0,
        "ssh",
        &format!(
            "echo \"$*\" >> {}\ncase \"$*\" in *' true') exit 0;; esac\nexec sh -s",
            calls.display()
        ),
    );
    stand_in(&s.0, "sshpass", "echo sshpass was used >&2; exit 9");
    let key = write_key(&s, 0o600);
    for verb in ["sh", "root"] {
        let (code, out, err) = run(&s, &key_device(&key), &["phone", verb], "echo over-key\n");
        assert_eq!((code, out.as_str()), (0, "over-key\n"), "{verb}: {err}");
        assert!(
            !err.contains("sshpass") && !err.contains("password"),
            "{err}"
        );
    }
    let said = std::fs::read_to_string(&calls).unwrap();
    let id = format!("-i {} -o IdentitiesOnly=yes", key.display());
    assert!(
        said.lines().all(|l| l.contains(&id)
            && l.contains("-o BatchMode=yes")
            && l.contains("root@10.9.9.8")),
        "{said}"
    );
}

#[test]
fn a_missing_or_world_readable_identity_is_refused_naming_the_path() {
    let s = Scratch::new("key-refused");
    stand_in(&s.0, "ssh", "exit 0");
    let missing = s.0.join("no_such_key");
    let (code, _, err) = run(&s, &key_device(&missing), &["phone", "sh"], "true\n");
    assert_ne!(code, 0);
    assert!(err.contains(&missing.display().to_string()), "{err}");
    let open = write_key(&s, 0o644);
    let (code, _, err) = run(&s, &key_device(&open), &["phone", "sh"], "true\n");
    assert_ne!(code, 0);
    assert!(
        err.contains(&open.display().to_string()) && err.contains("600"),
        "{err}"
    );
}

#[test]
fn a_key_phone_has_the_suite_and_asks_no_password() {
    let s = Scratch::new("key-suite");
    stand_in(&s.0, "ssh", "exit 1");
    let key = write_key(&s, 0o600);
    let (_, out, err) = run(&s, &key_device(&key), &["suite", "latest"], "");
    assert!(
        !out.contains("not applicable"),
        "a key login has a suite: {out}"
    );
    assert!(!err.contains("PHONE_PASS"), "no password was asked: {err}");
}

#[test]
fn root_over_a_key_is_the_login_itself_for_root_and_sudo_dash_n_for_another_user() {
    let s = Scratch::new("key-root");
    let calls = s.0.join("calls");
    stand_in(
        &s.0,
        "ssh",
        &format!(
            "echo \"$*\" >> {}\ncase \"$*\" in *' true') exit 0;; esac\nexec sh -s",
            calls.display()
        ),
    );
    let key = write_key(&s, 0o600);
    let (code, _, err) = run(&s, &key_device(&key), &["phone", "root"], "true\n");
    assert_eq!(code, 0, "{err}");
    let said = std::fs::read_to_string(&calls).unwrap();
    assert!(!said.contains("sudo") && said.contains(" sh -s"), "{said}");
    std::fs::remove_file(&calls).unwrap();
    let (code, _, err) = run(
        &s,
        &key_device_as(&key, "lab"),
        &["phone", "root"],
        "true\n",
    );
    assert_eq!(code, 0, "{err}");
    let said = std::fs::read_to_string(&calls).unwrap();
    assert!(
        said.contains("lab@10.9.9.8") && said.contains("sudo -n sh -s"),
        "{said}"
    );
}
