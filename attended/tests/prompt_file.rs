// SPDX-License-Identifier: MIT OR Apache-2.0
//! The person backend's second way to be answered: a pending prompt is written to DIR/prompt (mode 0600) and
//! DIR/answer is polled, the terminal answering too, whichever comes first. The procedure is the example, run by
//! rust-script against this checkout, with a stand-in phone.

use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-prompt-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    fs::write(
        d.join("device.toml"),
        "[device]\ncodename = \"xiaomi-surya\"\n",
    )
    .unwrap();
    d
}

fn example(dir: &Path) -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = fs::read_to_string(root.join("examples/attended.rs")).unwrap();
    let git = src
        .lines()
        .find(|l| l.starts_with("//! lpoco-attended = { git"))
        .unwrap();
    let local = format!("//! lpoco-attended = {{ path = \"{}\" }}", root.display());
    let f = dir.join("attended.rs");
    fs::write(&f, src.replace(git, &local)).unwrap();
    f
}

/// A stand-in phone that answers every call with sh.
fn devlab(dir: &Path) -> PathBuf {
    let bin = dir.join("phone/bin");
    fs::create_dir_all(&bin).unwrap();
    fs::write(bin.join("uname"), "#!/bin/sh\necho 6.1.0-pmicwdt\n").unwrap();
    fs::write(bin.join("lsmod"), "#!/bin/sh\necho 'pmicwdt 16384 0'\n").unwrap();
    let d = dir.join("devlab");
    fs::write(
        &d,
        format!("#!/bin/sh\nPATH={}:$PATH exec sh -s\n", bin.display()),
    )
    .unwrap();
    Command::new("chmod")
        .arg("+x")
        .args([&d, &bin.join("uname"), &bin.join("lsmod")])
        .status()
        .unwrap();
    d
}

fn spawn(dir: &Path, stdin: Stdio, extra: &[(&str, &str)]) -> Child {
    let mut c = Command::new("rust-script");
    c.arg(example(dir))
        .args(["--log", dir.join("run.log").to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", devlab(dir))
        .env("LPOCO_ATTENDED_PROMPT_DIR", dir.join("prompts"))
        .current_dir(dir)
        .stdin(stdin)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for (k, v) in extra {
        c.env(k, v);
    }
    c.spawn().expect("rust-script is on the lab machine")
}

fn wait_for(path: &Path) {
    let until = Instant::now() + Duration::from_secs(120);
    while !path.exists() {
        assert!(Instant::now() < until, "{} never appeared", path.display());
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn log(dir: &Path) -> String {
    fs::read_to_string(dir.join("run.log")).unwrap()
}

#[test]
fn a_pending_prompt_is_filed_and_an_answer_file_is_taken_and_logged_with_who_wrote_it() {
    let dir = tmp("file");
    let dev = dir.join("prompts/xiaomi-surya");
    let c = spawn(&dir, Stdio::piped(), &[]);
    wait_for(&dev.join("prompt"));
    let p = fs::read_to_string(dev.join("prompt")).unwrap();
    assert!(
        p.contains("do: hold volume down and plug the cable in"),
        "{p}"
    );
    assert!(
        p.contains("check: the screen shows the fastboot logo"),
        "{p}"
    );
    assert!(
        p.contains("waits-until: ")
            && !p.contains("waits-until: none")
            && p.contains("state: waiting"),
        "{p}"
    );
    assert_eq!(
        fs::metadata(dev.join("prompt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::write(dev.join("answer"), "ok\nby: top-coordinator\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let l = log(&dir);
    assert!(
        l.contains("answer: ok") && l.contains("written by top-coordinator"),
        "{l}"
    );
    assert!(!dev.join("prompt").exists() && !dev.join("answer").exists());
}

#[test]
fn the_terminal_still_answers_and_the_files_are_gone_after() {
    let dir = tmp("terminal");
    let mut c = spawn(&dir, Stdio::piped(), &[]);
    c.stdin.take().unwrap().write_all(b"ok\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(
        log(&dir).contains("answer: ok; by: terminal"),
        "{}",
        log(&dir)
    );
    assert!(!dir.join("prompts/xiaomi-surya/prompt").exists());
}

#[test]
fn a_wrong_answer_in_the_file_stops_the_run_naming_what_was_seen() {
    let dir = tmp("seen");
    let dev = dir.join("prompts/xiaomi-surya");
    let c = spawn(&dir, Stdio::piped(), &[]);
    wait_for(&dev.join("prompt"));
    fs::write(dev.join("answer"), "a blank screen\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("a blank screen"));
}

#[test]
fn a_prompt_waits_for_the_file_past_the_end_of_input_and_on_the_limit_says_timed_out_and_fails() {
    let dir = tmp("timeout");
    let c = spawn(&dir, Stdio::null(), &[("LPOCO_ATTENDED_ANSWER_LIMIT", "2")]);
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stderr).contains("nobody answered within 2 s"));
    let p = fs::read_to_string(dir.join("prompts/xiaomi-surya/prompt")).unwrap();
    assert!(p.contains("state: timed out"), "{p}");
}

#[test]
fn an_answer_left_from_before_is_never_taken_for_this_prompt() {
    let dir = tmp("stale");
    let dev = dir.join("prompts/xiaomi-surya");
    fs::create_dir_all(&dev).unwrap();
    fs::write(dev.join("answer"), "ok\n").unwrap();
    let c = spawn(&dir, Stdio::null(), &[("LPOCO_ATTENDED_ANSWER_LIMIT", "2")]);
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(1),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn an_answer_still_being_written_is_waited_for_not_taken_half() {
    let dir = tmp("half");
    let dev = dir.join("prompts/xiaomi-surya");
    let mut c = spawn(&dir, Stdio::piped(), &[]);
    wait_for(&dev.join("prompt"));
    fs::write(dev.join("answer"), "ok").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        c.try_wait().unwrap().is_none(),
        "the run took a half-written answer"
    );
    assert!(
        dev.join("answer").exists(),
        "the half-written answer was removed"
    );
    fs::write(dev.join("answer"), "ok\nby: patrol\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(log(&dir).contains("written by patrol"));
}

#[test]
fn an_answer_for_another_prompt_is_dropped_and_one_for_this_prompt_is_taken() {
    let dir = tmp("for");
    let dev = dir.join("prompts/xiaomi-surya");
    let mut c = spawn(&dir, Stdio::piped(), &[]);
    wait_for(&dev.join("prompt"));
    let id = fs::read_to_string(dev.join("prompt"))
        .unwrap()
        .lines()
        .find_map(|l| l.strip_prefix("id: ").map(str::to_string))
        .expect("the prompt file names its id");
    fs::write(dev.join("answer"), "ok\nfor: 1-99\n").unwrap();
    std::thread::sleep(Duration::from_millis(1500));
    assert!(
        c.try_wait().unwrap().is_none(),
        "an answer for another prompt was taken"
    );
    fs::write(dev.join("answer"), format!("ok\nfor: {id}\n")).unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn a_prompt_that_cannot_be_filed_with_input_ended_does_not_wait_out_the_limit() {
    let dir = tmp("unfiled");
    fs::write(dir.join("blocked"), "a file where the folder should be").unwrap();
    let began = Instant::now();
    let c = spawn(
        &dir,
        Stdio::null(),
        &[
            ("LPOCO_ATTENDED_ANSWER_LIMIT", "60"),
            (
                "LPOCO_ATTENDED_PROMPT_DIR",
                dir.join("blocked").to_str().unwrap(),
            ),
        ],
    );
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(
        began.elapsed() < Duration::from_secs(50),
        "waited out the limit"
    );
}

#[test]
fn with_no_terminal_a_prompt_waits_on_its_answer_file_by_default_and_says_so() {
    let dir = tmp("noterminal");
    let dev = dir.join("prompts/xiaomi-surya");
    // standard input at its end, no limit given anywhere: EOF is no answer
    let c = spawn(&dir, Stdio::null(), &[]);
    wait_for(&dev.join("prompt"));
    std::thread::sleep(Duration::from_secs(2));
    let p = fs::read_to_string(dev.join("prompt")).unwrap();
    assert!(
        p.contains("state: waiting"),
        "still waiting after 2 s of EOF: {p}"
    );
    let until: u64 = p
        .lines()
        .find_map(|l| l.strip_prefix("waits-until: "))
        .and_then(|u| u.trim().parse().ok())
        .expect("the default limit is filed as a time");
    let left = until.saturating_sub(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    );
    assert!(
        (200..=300).contains(&left),
        "the default is 300 s: {left} s left"
    );
    fs::write(dev.join("answer"), "ok\nby: top-coordinator\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let e = String::from_utf8_lossy(&o.stderr);
    assert!(e.contains("no terminal, waiting on the answer file"), "{e}");
    assert!(log(&dir).contains("written by top-coordinator"));
}
