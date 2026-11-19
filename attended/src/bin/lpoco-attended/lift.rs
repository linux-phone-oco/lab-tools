// SPDX-License-Identifier: MIT OR Apache-2.0
//! `lpoco attended lift`: every device repo's procedures/steps beside the library, near-duplicates flagged by
//! their bodies as token runs, never by name (decision 2026-11-19-procedures-reflect-and-lift).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// One public step function: where it lives, its name, and its body as a run of tokens (its own name left out).
pub struct Step {
    pub place: String,
    pub name: String,
    pub body: String,
}

/// One line of the answer: what to do, the step's names, and every `place:name` it was found at.
#[derive(Debug, PartialEq)]
pub struct Row {
    pub kind: &'static str,
    pub names: Vec<String>,
    pub places: Vec<String>,
}

const REPO_STEPS: &str = "procedures/steps";
/// Copies that differ lift at three repos (the rule of three).
const DIFFERING_AT: usize = 3;

/// Every step of the repos' procedures/steps and, apart, of the library folder.
pub fn read(repos: &[PathBuf], library: Option<&Path>) -> Result<(Vec<Step>, Vec<Step>), String> {
    let mut mine = Vec::new();
    for repo in repos {
        let place = repo
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .ok_or_else(|| format!("{} is not a repo folder", repo.display()))?;
        if mine.iter().any(|s: &Step| s.place == place) {
            return Err(format!(
                "two repos are both called {place}: a place is its folder's name, so rename one"
            ));
        }
        steps_in(&repo.join(REPO_STEPS), &place, &mut mine)?;
    }
    let mut lib = Vec::new();
    if let Some(dir) = library {
        steps_in(dir, "library", &mut lib)?;
    }
    Ok((mine, lib))
}

fn steps_in(dir: &Path, place: &str, out: &mut Vec<Step>) -> Result<(), String> {
    let rd = std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok()).map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        let name = p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if p.is_dir() && name != "target" && !name.starts_with('.') {
            steps_in(&p, place, out)?;
        } else if p.is_file() && name.ends_with(".rs") {
            let text = std::fs::read_to_string(&p)
                .map_err(|e| format!("cannot read {}: {e}", p.display()))?;
            out.extend(
                public_fns(&text)
                    .map_err(|e| format!("{}: {e}", p.display()))?
                    .into_iter()
                    .map(|(n, body)| Step {
                        place: place.into(),
                        name: n,
                        body,
                    }),
            );
        }
    }
    Ok(())
}

/// The tokens of Rust text without comments: words, string literals whole, every other character alone.
fn tokens(text: &str) -> Result<Vec<String>, String> {
    let c: Vec<char> = text.chars().collect();
    let (mut i, mut out) = (0, Vec::new());
    while i < c.len() {
        let ch = c[i];
        if ch.is_whitespace() {
            i += 1;
        } else if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 0;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
        } else if ch == 'r' && raw_string_at(&c, i).is_some() {
            let end = raw_string_at(&c, i).unwrap_or(c.len());
            out.push(c[i..end].iter().collect());
            i = end;
        } else if ch == '\'' && char_literal_end(&c, i).is_some() {
            let end = char_literal_end(&c, i).unwrap_or(c.len());
            out.push(c[i..end].iter().collect());
            i = end;
        } else if ch == '"' {
            let start = i;
            i += 1;
            while i < c.len() && c[i] != '"' {
                i += if c[i] == '\\' { 2 } else { 1 };
            }
            if c.get(i) != Some(&'"') {
                return Err("a string is never closed".into());
            }
            i += 1;
            out.push(c[start..i].iter().collect());
        } else if ch.is_alphanumeric() || ch == '_' {
            let start = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') {
                i += 1;
            }
            out.push(c[start..i].iter().collect());
        } else {
            out.push(ch.to_string());
            i += 1;
        }
    }
    Ok(out)
}

/// The index of the brace that closes the one opened at `open`.
fn closing(t: &[String], open: usize) -> Result<usize, String> {
    let mut d = 0;
    for (k, tok) in t.iter().enumerate().skip(open) {
        match tok.as_str() {
            "{" => d += 1,
            "}" => {
                d -= 1;
                if d == 0 {
                    return Ok(k);
                }
            }
            _ => {}
        }
    }
    Err("a brace is never closed".into())
}

/// Each top-level `pub fn`: its name and everything after the name up to its closing brace, as one string.
pub fn public_fns(text: &str) -> Result<Vec<(String, String)>, String> {
    let t = tokens(text)?;
    let (mut i, mut depth, mut out) = (0, 0i32, Vec::new());
    while i < t.len() {
        match t[i].as_str() {
            "{" => depth += 1,
            "}" => depth -= 1,
            "pub"
                if depth == 0
                    && t.get(i + 1).map(String::as_str) == Some("fn")
                    && i + 2 < t.len() =>
            {
                let name = t[i + 2].clone();
                let mut j = i + 3;
                while j < t.len() && t[j] != "{" && t[j] != ";" {
                    j += 1;
                }
                if j < t.len() && t[j] == "{" {
                    let k = closing(&t, j)?;
                    out.push((name, t[i + 3..=k].join(" ")));
                    i = k;
                }
            }
            _ => {}
        }
        i += 1;
    }
    Ok(out)
}

fn at(s: &Step) -> String {
    format!("{}:{}", s.place, s.name)
}

fn distinct_places(group: &[&Step]) -> usize {
    let mut p: Vec<&str> = group.iter().map(|s| s.place.as_str()).collect();
    p.sort();
    p.dedup();
    p.len()
}

fn names_of(group: &[&Step]) -> Vec<String> {
    let mut n: Vec<String> = group.iter().map(|s| s.name.clone()).collect();
    n.sort();
    n.dedup();
    n
}

fn group_row(kind: &'static str, group: &[&Step]) -> Row {
    Row {
        kind,
        names: names_of(group),
        places: group.iter().map(|s| at(s)).collect(),
    }
}

/// What to do with each step: use the library's; lift one copied into two repos; lift one that differs in
/// three; or only list it.
pub fn lift(mine: &[Step], library: &[Step]) -> Vec<Row> {
    let mut rows = Vec::new();
    let mut used = vec![false; mine.len()];
    for (i, s) in mine.iter().enumerate() {
        let found: Vec<&Step> = library.iter().filter(|l| l.body == s.body).collect();
        if !found.is_empty() {
            used[i] = true;
            let mut places = vec![at(s)];
            places.extend(found.iter().map(|l| at(l)));
            rows.push(Row {
                kind: "use-library",
                names: vec![s.name.clone()],
                places,
            });
        }
    }
    let mut by_name: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in mine.iter().enumerate().filter(|(i, _)| !used[*i]) {
        by_name.entry(s.name.as_str()).or_default().push(i);
    }
    for idx in by_name.values() {
        let group: Vec<&Step> = idx.iter().map(|&i| &mine[i]).collect();
        let differ = group.iter().any(|s| s.body != group[0].body);
        if differ && distinct_places(&group) >= DIFFERING_AT {
            idx.iter().for_each(|&i| used[i] = true);
            rows.push(group_row("lift-differing", &group));
        }
    }
    let mut by_body: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (i, s) in mine.iter().enumerate().filter(|(i, _)| !used[*i]) {
        by_body.entry(s.body.as_str()).or_default().push(i);
    }
    for idx in by_body.values() {
        let group: Vec<&Step> = idx.iter().map(|&i| &mine[i]).collect();
        if distinct_places(&group) >= 2 {
            idx.iter().for_each(|&i| used[i] = true);
            rows.push(group_row("lift", &group));
        }
    }
    for (_, s) in mine.iter().enumerate().filter(|(i, _)| !used[*i]) {
        rows.push(group_row("listed", &[s]));
    }
    rows
}

/// The rows as csv: kind, the step's names, its places (names and places joined by `|`).
pub fn csv(rows: &[Row]) -> String {
    let mut out = String::from("kind,step,places\n");
    for r in rows {
        out.push_str(&format!(
            "{},{},{}\n",
            r.kind,
            r.names.join("|"),
            r.places.join("|")
        ));
    }
    out
}

/// The end of a raw string `r"…"` / `r#"…"#` starting at `i`, when one starts there.
fn raw_string_at(c: &[char], i: usize) -> Option<usize> {
    let mut j = i + 1;
    while c.get(j) == Some(&'#') {
        j += 1;
    }
    if c.get(j) != Some(&'"') {
        return None;
    }
    let hashes = j - i - 1;
    let close: Vec<char> = std::iter::once('"')
        .chain(std::iter::repeat_n('#', hashes))
        .collect();
    (j + 1..c.len())
        .find(|&k| c[k..].starts_with(&close))
        .map(|k| k + close.len())
}

/// The end of a character literal `'x'` / `'\n'` starting at `i`; a lifetime (`'a`) is not one.
fn char_literal_end(c: &[char], i: usize) -> Option<usize> {
    match (c.get(i + 1)?, c.get(i + 2)) {
        ('\\', _) => (i + 3..c.len().min(i + 12))
            .find(|&k| c[k] == '\'')
            .map(|k| k + 1),
        (_, Some('\'')) => Some(i + 3),
        _ => None,
    }
}
