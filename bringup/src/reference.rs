//! The reference snapshot: what a stock OS does for a part, taken once over a transport (adb, ssh) and
//! compared with the same snapshot of our mainline boot. A snapshot is a folder: `devicetree.txt`
//! (one `node property = value` line per property, sorted, phandles by name only, the format walleye's
//! by-hand UFS comparison used) and the phone's text files as they are (`gpio`, `regulator_summary`,
//! `clk_summary`, `interrupts`, `pinctrl`, `drivers`).
//!
//! `capture` runs [`CAPTURE_SH`] on the phone through the transport and writes the folder; `diff` filters
//! two folders to one part and names each difference. What a side does not have is "not measured yet".

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

pub const CAPTURE_SH: &str = include_str!("reference.sh");

/// The text files a snapshot may hold; one a phone did not give is not measured yet.
pub const FILES: &[&str] = &[
    "gpio",
    "pinctrl",
    "regulator_summary",
    "clk_summary",
    "interrupts",
    "drivers",
];

/// Properties whose cells are numbers of the tree (phandles), which differ by tree and are named only.
fn is_reference(prop: &str) -> bool {
    matches!(
        prop,
        "clocks"
            | "phys"
            | "resets"
            | "power-domains"
            | "interrupt-parent"
            | "phandle"
            | "ufs-qcom-crypto-ref"
            | "iommus"
            | "dmas"
            | "mboxes"
            | "interconnects"
    ) || prop.ends_with("-supply")
        || prop.ends_with("-fixed-regulator")
        || prop
            .strip_prefix("pinctrl-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

/// One property's bytes as a line value: strings quoted, cells in hex, a phandle by name only.
pub fn value(prop: &str, bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "(empty)".into();
    }
    if is_reference(prop) {
        return "(phandle or reference)".into();
    }
    let printable = bytes.last() == Some(&0)
        && bytes.iter().any(|&b| b != 0)
        && !bytes.contains(&0xff)
        && bytes.iter().all(|&b| b == 0 || (32..127).contains(&b))
        // consecutive NULs are padding of cells, not an empty string
        && !bytes.windows(2).any(|w| w == [0, 0]);
    if printable {
        let strs: Vec<String> = bytes[..bytes.len() - 1]
            .split(|&b| b == 0)
            .map(|s| format!("\"{}\"", String::from_utf8_lossy(s)))
            .collect();
        return strs.join(",");
    }
    if bytes.len().is_multiple_of(4) {
        let cells: Vec<String> = bytes
            .chunks(4)
            .map(|c| format!("0x{:x}", u32::from_be_bytes([c[0], c[1], c[2], c[3]])))
            .collect();
        return format!("<{}>", cells.join(" "));
    }
    let hex: Vec<String> = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("[{}]", hex.join(" "))
}

/// What the phone said: device-tree properties (path, bytes) and text files (name, text).
#[derive(Debug, Default)]
pub struct Capture {
    pub props: Vec<(String, Vec<u8>)>,
    pub files: Vec<(String, String)>,
}

/// Read the capture script's output.
pub fn parse(output: &str) -> Result<Capture, String> {
    let mut c = Capture::default();
    enum In {
        Nothing,
        Dt(String, Vec<u8>),
        File(String, String),
    }
    let mut cur = In::Nothing;
    let close = |cur: In, c: &mut Capture| match cur {
        In::Dt(p, b) => c.props.push((p, b)),
        In::File(n, t) => c.files.push((n, t)),
        In::Nothing => {}
    };
    for line in output.lines() {
        if let Some(path) = line.strip_prefix("@@dt ") {
            close(
                std::mem::replace(&mut cur, In::Dt(path.trim().into(), vec![])),
                &mut c,
            );
        } else if let Some(name) = line.strip_prefix("@@file ") {
            close(
                std::mem::replace(&mut cur, In::File(name.trim().into(), String::new())),
                &mut c,
            );
        } else {
            match &mut cur {
                In::Dt(p, bytes) => {
                    for h in line.split_whitespace() {
                        bytes.push(
                            u8::from_str_radix(h, 16)
                                .map_err(|_| format!("{p}: {h:?} is not a hex byte"))?,
                        );
                    }
                }
                In::File(_, t) => {
                    t.push_str(line);
                    t.push('\n');
                }
                In::Nothing => {}
            }
        }
    }
    close(cur, &mut c);
    if c.props.is_empty() {
        return Err("the phone gave no device tree (is /proc/device-tree readable? is the transport a shell?)".into());
    }
    Ok(c)
}

/// The `node property = value` lines, sorted: the node is the last folder of the path without its unit address.
pub fn normalise(c: &Capture) -> String {
    let mut lines: Vec<String> = c
        .props
        .iter()
        .filter_map(|(path, bytes)| {
            let (dir, prop) = path.rsplit_once('/')?;
            if prop == "name" {
                return None;
            }
            let node = dir.rsplit('/').next().filter(|n| *n != ".")?;
            let node = node.split('@').next().unwrap_or(node);
            Some(format!("{node} {prop} = {}", value(prop, bytes)))
        })
        .collect();
    lines.sort();
    lines.into_iter().map(|l| l + "\n").collect()
}

/// Run the capture script on the phone through `via` (a command and its arguments, split at spaces; the
/// script goes in on its standard input) and write the snapshot folder. Returns the lines to say.
pub fn capture(via: &str, out: &Path) -> Result<Vec<String>, String> {
    let mut words = via.split_whitespace();
    let prog = words
        .next()
        .ok_or("--via needs a transport command (adb shell, ssh HOST)")?;
    let mut child = Command::new(prog)
        .args(words)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| format!("cannot run {prog}: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let script = CAPTURE_SH.to_string();
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(script.as_bytes());
    });
    let o = child
        .wait_with_output()
        .map_err(|e| format!("{prog}: {e}"))?;
    let _ = writer.join();
    if !o.status.success() {
        return Err(format!("{via}: exited {}", o.status));
    }
    let cap = parse(&String::from_utf8_lossy(&o.stdout))?;
    fs::create_dir_all(out).map_err(|e| format!("cannot make {}: {e}", out.display()))?;
    let put = |name: &str, text: &str| {
        fs::write(out.join(name), text)
            .map_err(|e| format!("cannot write {}/{name}: {e}", out.display()))
    };
    let dt = normalise(&cap);
    put("devicetree.txt", &dt)?;
    let mut said = vec![format!(
        "{}/devicetree.txt: {} properties",
        out.display(),
        dt.lines().count()
    )];
    for name in FILES {
        match cap.files.iter().find(|(n, _)| n == name) {
            Some((_, t)) => {
                put(name, t)?;
                said.push(format!(
                    "{}/{name}: {} lines",
                    out.display(),
                    t.lines().count()
                ));
            }
            None => {
                let _ = fs::remove_file(out.join(name));
                said.push(format!("not measured yet: {name} (the phone did not give it; root and debugfs may be needed)"));
            }
        }
    }
    Ok(said)
}

/// The two snapshots compared for one part.
pub struct Report {
    pub lines: Vec<String>,
    /// Lines of the part found in either snapshot; 0 means the part is not known yet.
    pub matched: usize,
    pub differences: usize,
    /// Things one side did not give: not known yet, so never a pass.
    pub unmeasured: usize,
}

fn mine(words: &[String], s: &str) -> bool {
    let low = s.to_lowercase();
    words.iter().any(|w| low.contains(w.as_str()))
}

/// Compare `live` (the stock OS) and `ours` for the part named by `words` (lower-case substrings; a
/// device-tree line belongs to the part when its node name holds one, a text line when it holds one).
pub fn diff(live: &Path, ours: &Path, words: &[String]) -> Result<Report, String> {
    let mut r = Report {
        lines: vec![],
        matched: 0,
        differences: 0,
        unmeasured: 0,
    };
    for d in [live, ours] {
        if !d.is_dir() {
            return Err(format!("{}: no such snapshot folder", d.display()));
        }
    }
    let read = |d: &Path, f: &str| fs::read_to_string(d.join(f)).ok();
    let (lt, ot) = (read(live, "devicetree.txt"), read(ours, "devicetree.txt"));
    match (&lt, &ot) {
        (Some(l), Some(o)) => {
            let split = |t: &str| -> Vec<(String, String, String)> {
                t.lines()
                    .filter_map(|l| {
                        let (node, rest) = l.split_once(' ')?;
                        let (prop, val) = rest.split_once(" = ")?;
                        mine(words, node)
                            .then(|| (node.to_string(), prop.to_string(), val.to_string()))
                    })
                    .collect()
            };
            let (a, b) = (split(l), split(o));
            let mut keys: Vec<(&str, &str)> = a
                .iter()
                .chain(b.iter())
                .map(|(n, p, _)| (n.as_str(), p.as_str()))
                .collect();
            keys.sort();
            keys.dedup();
            // a node name can repeat (cpu, regulator): the values of one key are compared as a sorted list
            let get = |v: &[(String, String, String)], n: &str, p: &str| -> Vec<String> {
                let mut o: Vec<String> = v
                    .iter()
                    .filter(|(x, y, _)| x == n && y == p)
                    .map(|t| t.2.clone())
                    .collect();
                o.sort();
                o
            };
            r.matched += keys.len();
            for (n, p) in keys {
                let (x, y) = (get(&a, n, p), get(&b, n, p));
                if x == y {
                    continue;
                }
                let (x, y) = (x.join(" ; "), y.join(" ; "));
                r.lines.push(match (x.is_empty(), y.is_empty()) {
                    (false, true) => format!("only live: devicetree {n} {p} = {x}"),
                    (true, false) => format!("only ours: devicetree {n} {p} = {y}"),
                    _ => format!("differs: devicetree {n} {p}: live {x} | ours {y}"),
                });
            }
        }
        _ => r.lines.push(format!(
            "devicetree: not measured yet on the {} side",
            if lt.is_none() { "live" } else { "ours" }
        )),
    }
    for f in FILES {
        let (l, o) = (read(live, f), read(ours, f));
        let (l, o) = match (l, o) {
            (Some(l), Some(o)) => (l, o),
            (None, None) => continue,
            (l, _) => {
                r.lines.push(format!(
                    "{f}: not measured yet on the {} side",
                    if l.is_none() { "live" } else { "ours" }
                ));
                continue;
            }
        };
        let squash = |t: &str| -> Vec<String> {
            let mut v: Vec<String> = t
                .lines()
                .filter(|l| mine(words, l))
                .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
                .collect();
            v.sort();
            v
        };
        let (a, b) = (squash(&l), squash(&o));
        r.matched += a.len().max(b.len());
        for x in a.iter().filter(|x| !b.contains(x)) {
            r.lines.push(format!("only live: {f}: {x}"));
        }
        for y in b.iter().filter(|y| !a.contains(y)) {
            r.lines.push(format!("only ours: {f}: {y}"));
        }
    }
    r.unmeasured = r
        .lines
        .iter()
        .filter(|l| l.contains(": not measured yet"))
        .count();
    r.differences = r.lines.len() - r.unmeasured;
    Ok(r)
}
