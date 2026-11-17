//! `lpoco mcp`: one MCP server for the family (backlog lpoco-mcp-one-door, proof
//! lpoco-mcp-lists-every-family-tool). Every subcommand's tools are listed under its name, and the
//! names devlab's door had before still answer.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};

/// The family's programs, built beside each other in the workspace's target folder.
fn family_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_BIN_EXE_lpoco"))
        .parent()
        .unwrap()
        .to_path_buf();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let built = Command::new(cargo)
        .args(["build", "--workspace", "--bins"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap();
    assert!(built.success(), "cargo build --workspace --bins failed");
    dir
}

/// Speak MCP to `lpoco mcp` over stdio; returns the result of each request in order.
fn ask(requests: &[(&str, serde_json::Value)]) -> Vec<serde_json::Value> {
    let dir = family_dir();
    let work = std::env::temp_dir().join(format!("lpoco-mcp-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).unwrap();
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../bringup/tests/fixtures/device.toml"
        ),
        work.join("device.toml"),
    )
    .unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_lpoco"))
        .arg("mcp")
        .current_dir(&work)
        .env("PATH", &dir)
        .env("HOME", &work)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut out = BufReader::new(child.stdout.take().unwrap());
    let mut send = |v: serde_json::Value| {
        writeln!(stdin, "{v}").unwrap();
        stdin.flush().unwrap();
    };
    let mut read = |id: u64| loop {
        let mut line = String::new();
        assert!(
            out.read_line(&mut line).unwrap() > 0,
            "lpoco mcp ended early"
        );
        let v: serde_json::Value = serde_json::from_str(&line).unwrap();
        if v["id"] == id {
            return v;
        }
    };
    send(
        serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}),
    );
    read(1);
    send(serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"}));
    let mut answers = Vec::new();
    for (i, (method, params)) in requests.iter().enumerate() {
        let id = 10 + i as u64;
        send(serde_json::json!({"jsonrpc":"2.0","id":id,"method":method,"params":params}));
        answers.push(read(id));
    }
    drop(stdin);
    let _ = child.wait();
    answers
}

fn names(list: &serde_json::Value) -> Vec<String> {
    list["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("no tools in {list}"))
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn lpoco_mcp_lists_every_family_tool() {
    let answers = ask(&[
        ("tools/list", serde_json::json!({})),
        (
            "tools/call",
            serde_json::json!({"name":"bringup_image_check","arguments":{"image":"/nonexistent.img"}}),
        ),
    ]);
    let got = names(&answers[0]);
    // each program's tools under its name
    for want in [
        "lab_run",
        "lab_put",
        "lab_get",
        "lab_kernel_install",
        "lab_kernel_try",
        "lab_kernel_status",
        "lab_reboot",
        "health_report",
        "health_run",
        "health_switches",
        "bringup_image_info",
        "bringup_image_check",
    ] {
        assert!(got.iter().any(|n| n == want), "{want} missing from {got:?}");
    }
    // the old names still answer
    for old in [
        "run",
        "put",
        "get",
        "kernel_install",
        "kernel_try",
        "kernel_status",
        "reboot",
    ] {
        assert!(
            got.iter().any(|n| n == old),
            "old name {old} missing from {got:?}"
        );
    }
    // a bringup tool really runs the family's program: a missing image is its error, not "tool not found"
    let text = answers[1].to_string();
    assert!(!text.contains("tool not found"), "{text}");
    assert!(text.contains("nonexistent"), "{text}");
}
