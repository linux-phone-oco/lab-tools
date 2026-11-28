// SPDX-License-Identifier: MIT OR Apache-2.0
//! The author is reached directly: a filed prompt is also published to his ntfy topic, and a reply on the reply
//! topic that names the prompt's id becomes the answer. The server here is a stand-in that speaks ntfy's two
//! calls (publish: POST /TOPIC; poll: GET /TOPIC/json?poll=1, one JSON message per line - docs.ntfy.sh/subscribe/api
//! and docs.ntfy.sh/publish); the procedure is the example, run by rust-script against this checkout.

use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// What the stand-in saw: each publish as (path, headers, body), and the replies it will hand out.
#[derive(Default)]
struct Seen {
    posts: Vec<(String, String, String)>,
    replies: Vec<String>,
}

fn serve() -> (u16, Arc<Mutex<Seen>>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let seen = Arc::new(Mutex::new(Seen::default()));
    let s = seen.clone();
    std::thread::spawn(move || {
        for conn in l.incoming().flatten() {
            let s = s.clone();
            std::thread::spawn(move || handle(conn, &s));
        }
    });
    (port, seen)
}

fn handle(conn: TcpStream, seen: &Mutex<Seen>) {
    let mut r = BufReader::new(conn.try_clone().unwrap());
    let mut first = String::new();
    r.read_line(&mut first).unwrap();
    let (mut headers, mut len) = (String::new(), 0usize);
    loop {
        let mut h = String::new();
        r.read_line(&mut h).unwrap();
        if h.trim().is_empty() {
            break;
        }
        if let Some(v) = h.to_lowercase().strip_prefix("content-length:") {
            len = v.trim().parse().unwrap();
        }
        headers.push_str(&h);
    }
    let mut body = vec![0; len];
    r.read_exact(&mut body).unwrap();
    let mut parts = first.split_whitespace();
    let (method, path) = (parts.next().unwrap(), parts.next().unwrap().to_string());
    let out = if method == "POST" {
        seen.lock().unwrap().posts.push((
            path,
            headers,
            String::from_utf8_lossy(&body).into_owned(),
        ));
        "{}".to_string()
    } else {
        let s = seen.lock().unwrap();
        s.replies
            .iter()
            .enumerate()
            .map(|(i, m)| {
                format!("{{\"id\":\"m{i}\",\"time\":1,\"event\":\"message\",\"topic\":\"reply\",\"message\":\"{m}\"}}\n")
            })
            .collect()
    };
    let mut c = conn;
    write!(
        c,
        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}",
        out.len()
    )
    .unwrap();
}

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("lpoco-notify-{}-{name}", std::process::id()));
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

/// The notify file a lab keeps (mode 600): where the server is, the two topics, an access token.
fn notify_file(dir: &Path, port: u16) -> PathBuf {
    let f = dir.join("notify");
    fs::write(&f, format!("server: http://127.0.0.1:{port}\ntopic: author\nreply_topic: reply\ntoken: tk_secret\n")).unwrap();
    fs::set_permissions(&f, fs::Permissions::from_mode(0o600)).unwrap();
    f
}

fn spawn(dir: &Path, notify: Option<&Path>) -> Child {
    let mut c = Command::new("rust-script");
    c.arg(example(dir))
        .args(["--log", dir.join("run.log").to_str().unwrap()])
        .env("LPOCO_ATTENDED_DEVLAB", devlab(dir))
        .env("LPOCO_ATTENDED_PROMPT_DIR", dir.join("prompts"))
        .env("LPOCO_ATTENDED_ANSWER_LIMIT", "60")
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(n) = notify {
        c.env("LPOCO_ATTENDED_NOTIFY", n);
    } else {
        c.env("LPOCO_ATTENDED_NOTIFY", dir.join("no-such-notify-file"));
    }
    c.spawn().expect("rust-script is on the lab machine")
}

fn until<T>(what: &str, mut f: impl FnMut() -> Option<T>) -> T {
    let end = Instant::now() + Duration::from_secs(120);
    loop {
        if let Some(v) = f() {
            return v;
        }
        assert!(Instant::now() < end, "{what} never happened");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn prompt_id(dir: &Path) -> String {
    let p = dir.join("prompts/xiaomi-surya/prompt");
    until("the prompt file", || {
        fs::read_to_string(&p)
            .ok()?
            .lines()
            .find_map(|l| l.strip_prefix("id: ").map(str::to_string))
    })
}

#[test]
fn a_filed_prompt_is_published_once_and_a_reply_for_its_id_is_the_answer_naming_ntfy() {
    let dir = tmp("reply");
    let (port, seen) = serve();
    let c = spawn(&dir, Some(&notify_file(&dir, port)));
    let id = prompt_id(&dir);
    let (path, headers, body) = until("the publish", || {
        seen.lock().unwrap().posts.first().cloned()
    });
    assert_eq!(path, "/author");
    assert!(
        body.contains("hold volume down and plug the cable in"),
        "{body}"
    );
    assert!(
        body.contains("the screen shows the fastboot logo"),
        "{body}"
    );
    assert!(body.contains("until"), "{body}");
    assert!(
        headers
            .to_lowercase()
            .contains("authorization: bearer tk_secret"),
        "{headers}"
    );
    // the reply button posts the id and the answer to the reply topic
    assert!(
        headers.contains("Actions:") && headers.contains(&format!("{id} ok")),
        "{headers}"
    );
    // a reply meant for another prompt is ignored; the one for this prompt is the answer
    seen.lock()
        .unwrap()
        .replies
        .push("0-0 a blank screen".to_string());
    std::thread::sleep(Duration::from_secs(5));
    seen.lock().unwrap().replies.push(format!("{id} ok"));
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let log = fs::read_to_string(dir.join("run.log")).unwrap();
    assert!(log.contains("answer: ok; by: ntfy"), "{log}");
    assert_eq!(
        seen.lock().unwrap().posts.len(),
        1,
        "one publish per prompt"
    );
}

#[test]
fn with_no_notify_file_the_run_says_so_and_waits_on_the_answer_file_as_before() {
    let dir = tmp("none");
    let c = spawn(&dir, None);
    let id = prompt_id(&dir);
    assert!(!id.is_empty());
    fs::write(
        dir.join("prompts/xiaomi-surya/answer"),
        "ok\nby: top-coordinator\n",
    )
    .unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(
        o.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("no notify file"), "{err}");
}

#[test]
fn a_notify_file_others_can_read_is_refused_saying_so_and_the_run_goes_on_without_it() {
    let dir = tmp("mode");
    let (port, seen) = serve();
    let f = notify_file(&dir, port);
    fs::set_permissions(&f, fs::Permissions::from_mode(0o644)).unwrap();
    let c = spawn(&dir, Some(&f));
    prompt_id(&dir);
    fs::write(dir.join("prompts/xiaomi-surya/answer"), "ok\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    let err = String::from_utf8_lossy(&o.stderr);
    assert!(err.contains("mode 600"), "{err}");
    assert!(seen.lock().unwrap().posts.is_empty());
}
