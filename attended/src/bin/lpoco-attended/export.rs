// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco-attended export NAME`: a procedure's checks as a LAVA test definition ("Lava-Test Test Definition 1.0",
//! <https://docs.lavasoftware.org/lava/technical-references/job-definition/actions/test.html>), for others' labs
//! to run. The procedure walks itself with `--export` (a dry run that also runs each check, its phone scripts
//! logged and not sent); this reads that log. A check's own judgement of what the phone answered stays in the
//! procedure: the exported test case passes when its script exits 0.

/// One check of the procedure: where it came from and the scripts it sends the phone.
#[derive(Debug, PartialEq)]
pub struct Case {
    pub path: String,
    pub bring_up_step: Option<String>,
    pub source: Option<String>,
    pub scripts: Vec<String>,
}

impl Case {
    /// The check's own name: the last of the groups it is in.
    fn name(&self) -> &str {
        self.path.rsplit(" > ").next().unwrap_or(&self.path)
    }
}

/// The checks the log names, in order. `dry PATH: check [(bring-up step ID)] [[source S]]` opens one, the
/// `script PATH: TEXT` lines after it (a backslash written `\\`, a newline `\n`) belong to it.
pub fn cases(log: &str) -> Vec<Case> {
    let mut cases: Vec<Case> = Vec::new();
    for line in log.lines() {
        // `SECS WHAT REST`: the path of a step may hold ": ", so it is cut by what is known of each line
        let mut words = line.splitn(3, ' ');
        let (_, Some(what), Some(rest)) = (words.next(), words.next(), words.next()) else {
            continue;
        };
        match what {
            // `dry PATH: check ...`: the detail holds no ": ", so the last one ends the path
            "dry" => {
                let Some((path, detail)) = rest.rsplit_once(": ") else {
                    continue;
                };
                if detail.starts_with("check") {
                    cases.push(Case {
                        path: path.to_string(),
                        bring_up_step: between(detail, "(bring-up step ", ")"),
                        source: between(detail, "[source ", "]"),
                        scripts: Vec::new(),
                    });
                }
            }
            // a check's closure runs right after its `dry` line: its scripts belong to the last case
            "script" => {
                let Some(c) = cases.last_mut() else { continue };
                let Some(detail) = rest
                    .strip_prefix(c.path.as_str())
                    .and_then(|d| d.strip_prefix(": "))
                else {
                    continue;
                };
                let script = unescape(detail);
                // a check that is tried again sends the same script each time
                if c.scripts.last() != Some(&script) {
                    c.scripts.push(script);
                }
            }
            _ => {}
        }
    }
    cases
}

fn between(text: &str, open: &str, close: &str) -> Option<String> {
    let rest = text.split_once(open)?.1;
    Some(rest.split_once(close)?.0.to_string())
}

fn unescape(text: &str) -> String {
    let mut out = String::new();
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// A test case id LAVA accepts: lowercase words and digits joined by `-`.
fn case_id(name: &str) -> String {
    name.split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// A YAML double-quoted scalar.
fn quoted(text: &str) -> String {
    format!(
        "\"{}\"",
        text.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
    )
}

/// A script as one shell word run by `sh -c`.
fn sh_c(script: &str) -> String {
    format!("sh -c '{}'", script.trim_end().replace('\'', "'\\''"))
}

/// The LAVA test definition: metadata (the device the scripts were made for, its values already in them) and one `lava-test-case` step per check that sends
/// the phone a script. A check that sends none (it only looks at the host) is named in a comment, not made up.
pub fn definition(name: &str, codename: &str, cases: &[Case]) -> String {
    let mut y = String::new();
    y.push_str("# written by lpoco attended export; a test case passes when its script exits 0; what the procedure judged of the phone's answer is not exported\n");
    y.push_str("metadata:\n");
    y.push_str("  format: Lava-Test Test Definition 1.0\n");
    y.push_str(&format!("  name: {}\n", quoted(name)));
    y.push_str(&format!(
        "  description: {}\n",
        quoted(&format!(
            "the checks of the attended procedure {name} for {codename}"
        ))
    ));
    y.push_str(&format!("  devices:\n    - {}\n", quoted(codename)));
    y.push_str("run:\n  steps:\n");
    let mut used: Vec<String> = Vec::new();
    for c in cases {
        if c.scripts.is_empty() {
            y.push_str(&format!(
                "    # check {}: sends the phone no script, so it is not a test case\n",
                quoted(c.name())
            ));
            continue;
        }
        let mut id = case_id(c.name());
        let mut n = 1;
        while used.contains(&id) {
            n += 1;
            id = format!("{}-{n}", case_id(c.name()));
        }
        used.push(id.clone());
        if let Some(step) = &c.bring_up_step {
            y.push_str(&format!("    # bring-up step: {step}\n"));
        }
        if let Some(source) = &c.source {
            y.push_str(&format!("    # source of what it expects: {source}\n"));
        }
        let script = c
            .scripts
            .iter()
            .map(|s| sh_c(s))
            .collect::<Vec<_>>()
            .join(" && ");
        y.push_str(&format!(
            "    - {}\n",
            quoted(&format!("lava-test-case {id} --shell {script}"))
        ));
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_read_back_whole_with_its_newlines_and_backslashes() {
        assert_eq!(unescape("a\\nb\\\\c"), "a\nb\\c");
    }

    #[test]
    fn ids_are_lowercase_words() {
        assert_eq!(case_id("pmicwdt is bound!"), "pmicwdt-is-bound");
    }
}

#[cfg(test)]
mod log_tests {
    use super::*;

    #[test]
    fn a_check_whose_name_holds_a_colon_and_a_space_keeps_its_scripts() {
        let log = "1 dry group: a > dmesg says \"pmic: probed\": check (bring-up step s1) [source research:x]\n\
                   2 script group: a > dmesg says \"pmic: probed\": dmesg | grep 'x: y'\\nuname\n\
                   3 dry other: step\n";
        let c = cases(log);
        assert_eq!(c.len(), 1, "{c:?}");
        assert_eq!(c[0].path, "group: a > dmesg says \"pmic: probed\"");
        assert_eq!(c[0].name(), "dmesg says \"pmic: probed\"");
        assert_eq!(c[0].scripts, vec!["dmesg | grep 'x: y'\nuname".to_string()]);
        assert_eq!(c[0].source.as_deref(), Some("research:x"));
    }
}
