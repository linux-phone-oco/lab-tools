//! What the program tells its reader to type names the program lpoco-lab (proof: lpoco-names-sweep,
//! messages slice). A message is a line of code, not a comment; `guard.rs` and the old-name
//! sentences stay for later slices, and the aliases keep working.

const SUBCOMMANDS: [&str; 9] = [
    "flash", "provision", "gather", "suite", "fastboot", "kernel", "phone", "mcp", "watch",
];
const LATER: [&str; 1] = ["guard.rs"];

#[test]
fn messages_tell_the_reader_to_type_lpoco_lab() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/src");
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.ends_with(".rs") || LATER.contains(&name.as_str()) {
            continue;
        }
        let text = std::fs::read_to_string(&path).unwrap();
        for (n, line) in text.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }
            for sub in SUBCOMMANDS {
                if code.contains(&format!("devlab {sub}")) {
                    bad.push(format!("{name}:{}: {}", n + 1, code));
                }
            }
        }
    }
    assert!(bad.is_empty(), "old name in a message:\n{}", bad.join("\n"));
}

#[test]
fn help_calls_the_program_lpoco_lab() {
    let o = std::process::Command::new(env!("CARGO_BIN_EXE_lpoco-lab"))
        .args(["gather", "--help"])
        .output()
        .unwrap();
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(!out.contains("devlab's"), "{out}");
}
