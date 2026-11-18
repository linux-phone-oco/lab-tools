//! The README's commands use the lpoco names (proof: lpoco-names-sweep, README slice).
//! A command is an indented line or an inline code span that starts with an old program
//! name; the sentences that say what the old names are stay, so they are not commands.

const OLD: [&str; 3] = ["devlab", "tone", "devlab-health"];

fn readme() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../README.md");
    std::fs::read_to_string(path).unwrap()
}

fn starts_with_old_name(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    OLD.contains(&first)
}

#[test]
fn indented_command_lines_use_lpoco_names() {
    let bad: Vec<_> = readme()
        .lines()
        .enumerate()
        .filter(|(_, l)| l.starts_with("    ") && starts_with_old_name(l.trim_start()))
        .map(|(n, l)| format!("{}: {}", n + 1, l.trim()))
        .collect();
    assert!(
        bad.is_empty(),
        "old names in command lines:\n{}",
        bad.join("\n")
    );
}

#[test]
fn inline_commands_use_lpoco_names() {
    let mut bad = Vec::new();
    for (n, line) in readme().lines().enumerate() {
        // odd-numbered pieces between backticks are code spans
        for span in line.split('`').skip(1).step_by(2) {
            // a lone name is a mention ("`devlab` run the same code"), a command has arguments
            if span.contains(' ') && starts_with_old_name(span) {
                bad.push(format!("{}: `{}`", n + 1, span));
            }
        }
    }
    assert!(
        bad.is_empty(),
        "old names in inline commands:\n{}",
        bad.join("\n")
    );
}
