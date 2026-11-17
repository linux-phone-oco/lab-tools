//! Gathering: every report the phone keeps that the lab does not have yet, brought into the device's
//! lab repository (tests/results/RUN.jsonl, as each run was written), and every run not yet filed,
//! filed into the picture's tables - linux-phone-oco/docs' test_results, a row per test, and
//! test_runs, a row per run with its counts. Nothing is retyped: the rows are the report's lines.

use crate::phone::Phone;
use anyhow::{bail, Context, Result};

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

const ON_PHONE: &str = "/var/lib/devlab/health";

/// A report's name as the phone writes it: RUN.jsonl, RUN being YYYYMMDD-HHMMSS.
pub fn is_report(name: &str) -> bool {
    let Some(run) = name.strip_suffix(".jsonl") else {
        return false;
    };
    let b = run.as_bytes();
    b.len() == 15
        && b[8] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 8 || c.is_ascii_digit())
}

/// Brings every report the phone has and `results` lacks; the names brought.
pub fn fetch(phone: &Phone, results: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(results)?;
    let (_, listing) = phone.root(&format!("ls {ON_PHONE} 2>/dev/null\n"))?;
    let mut brought = Vec::new();
    for name in listing.lines().map(str::trim).filter(|n| is_report(n)) {
        let here = results.join(name);
        if here.exists() {
            continue;
        }
        let bytes = phone
            .root_bytes(&format!("cat {ON_PHONE}/{name}\n"))
            .with_context(|| format!("bringing {name}"))?;
        std::fs::write(&here, bytes)?;
        brought.push(name.to_string());
    }
    Ok(brought)
}

/// The statement that files one run into the picture's tables, through tad: every line of the report a
/// test_results row, and the run's test_runs row counted from them - a failure in one of the phone's
/// known gaps (its manifest's [health] gaps: parts it never had working) counted apart from the
/// failures, as the phone's own verdict counts it. tad keeps it only if every rule of the tables holds.
pub fn statement(run: &str, report: &Path, gaps: &[String]) -> String {
    let q = |t: &str| t.replace('\'', "''");
    let path = q(&report.to_string_lossy());
    let gaps: Vec<String> = gaps.iter().map(|g| format!("'{}'", q(g))).collect();
    let in_gap = format!(
        "list_contains([{}]::VARCHAR[], component_id)",
        gaps.join(", ")
    );
    format!(
        "INSERT INTO test_results SELECT '{run}-' || component || '-' || test, '{run}', device, component, test, \
         outcome, detail, CAST(\"at\" AS TIMESTAMP), kernel, shell \
         FROM read_json_auto('{path}', format = 'newline_delimited'); \
         INSERT INTO test_runs (id, device_id, run_at, kernel, shell, passed, failed, gaps, skipped) \
         SELECT run, any_value(device_id), min(run_at), any_value(kernel), any_value(shell), \
         count_if(outcome = 'pass'), count_if(outcome = 'fail' AND NOT {in_gap}), \
         count_if(outcome = 'fail' AND {in_gap}), count_if(outcome = 'skip') \
         FROM test_results WHERE run = '{run}' GROUP BY run"
    )
}

fn filed(docs: &Path) -> BTreeSet<String> {
    std::fs::read_to_string(docs.join("data/test_runs.json"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .filter_map(|v| v["id"].as_str().map(str::to_string))
        .collect()
}

fn tad(docs: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("tad")
        .args(args)
        .current_dir(docs)
        .output()
        .context("running tad (is it installed? linux-phone-oco/docs: bin/install-tad.sh)")?;
    if !out.status.success() {
        bail!(
            "tad {} refused: {}{}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(())
}

/// The statements the picture derives from its tables, in order: derive/*.sql. devlab knows none of them -
/// what a part's status is, is the picture's to say.
pub fn derivations(docs: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(docs.join("derive"))
        .map(|d| {
            d.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().is_some_and(|x| x == "sql"))
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// Files every run in `results` that the picture's database does not hold yet, through tad; then its
/// pages are rendered again, as its checks ask. The runs filed.
pub fn file(results: &Path, docs: &Path, gaps: &[String]) -> Result<Vec<String>> {
    let have = filed(docs);
    let mut reports: Vec<PathBuf> = std::fs::read_dir(results)
        .with_context(|| format!("reading {}", results.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(is_report)
        })
        .collect();
    reports.sort();
    let mut done = Vec::new();
    for p in reports {
        let run = p
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        if have.contains(&run) {
            continue;
        }
        let p = p.canonicalize()?;
        tad(docs, &["sql", &statement(&run, &p, gaps)]).with_context(|| format!("filing {run}"))?;
        done.push(run);
    }
    if !done.is_empty() {
        // what the picture derives from its runs - each statement it keeps in derive/ - follows them
        for rule in derivations(docs) {
            let rule = rule.to_string_lossy().into_owned();
            tad(docs, &["sql", "--file", &rule]).with_context(|| format!("deriving by {rule}"))?;
        }
        tad(docs, &["canon"])?;
        tad(docs, &["render"])?;
    }
    Ok(done)
}

/// Gathering, whole: the phone's reports brought, every unfiled run filed through tad. With `land`, in
/// devlab's own clones of the lab and the picture, each landed - and the face is drawn again by itself.
/// What it did, in words.
pub fn gather(
    device: &crate::device::Device,
    here: &Path,
    docs: Option<PathBuf>,
    land: bool,
) -> Result<String> {
    let docs = docs.unwrap_or_else(|| here.join("../docs"));
    let (lab, picture) = if land {
        let under = device.state_dir().join("places");
        (
            Some(Place::at_main(here, &under)?),
            Some(Place::at_main(&docs, &under)?),
        )
    } else {
        (None, None)
    };
    let lab_dir = lab.as_ref().map_or(here.to_path_buf(), |p| p.dir.clone());
    let docs_dir = picture.as_ref().map_or(docs, |p| p.dir.clone());
    let results = lab_dir.join("tests/results");
    let mut said = match Phone::find(device) {
        Ok(p) => format!("brought from the phone: {}", fetch(&p, &results)?.len()),
        Err(e) => format!("the phone is not reachable ({e:#}): filing what the lab has"),
    };
    let filed = file(&results, &docs_dir, &device.health.gaps)?;
    said += &format!(
        "; filed through tad: {} runs {}",
        filed.len(),
        filed.join(" ")
    );
    if let (Some(lab), Some(picture)) = (lab, picture) {
        let codename = &device.device.codename;
        let stamp = filed.last().cloned().unwrap_or_else(|| "reports".into());
        if lab.land(
            &format!("gathered/{stamp}"),
            &format!("{codename}'s own health reports, gathered"),
        )? {
            said += "; landed in the lab";
        }
        if picture.land(
            &format!("filed/{codename}-{stamp}"),
            &format!(
                "{} runs of {codename} filed by devlab, from the phone's own reports",
                filed.len()
            ),
        )? {
            said += "; landed in the picture - its face is drawn again by itself";
        }
    }
    Ok(said)
}

/// A place devlab lands into: its own clone, never one a person or an agent works in - two hands in one
/// clone once merged each other's changes under one commit.
pub struct Place {
    pub dir: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git").args(args).current_dir(dir).output()?;
    if !out.status.success() {
        bail!(
            "git {} in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&out.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

impl Place {
    /// devlab's own clone of the repository `of` is a clone of, at its main as GitHub has it now.
    pub fn at_main(of: &Path, under: &Path) -> Result<Place> {
        let url = git(of, &["remote", "get-url", "origin"])?;
        let name = url
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("place")
            .to_string();
        let dir = under.join(name);
        if !dir.join(".git").exists() {
            std::fs::create_dir_all(under)?;
            git(under, &["clone", "-q", &url, &dir.to_string_lossy()])?;
        }
        git(&dir, &["fetch", "-q", "origin"])?;
        git(&dir, &["checkout", "-q", "-f", "main"])?;
        git(&dir, &["reset", "-q", "--hard", "origin/main"])?;
        git(&dir, &["clean", "-q", "-fd"])?;
        Ok(Place { dir })
    }

    /// Lands what changed here: its own branch, the change said as a glimpse, and `land`, which waits
    /// for every check. Nothing changed, nothing landed.
    pub fn land(&self, branch: &str, summary: &str) -> Result<bool> {
        if git(&self.dir, &["status", "--porcelain"])?.is_empty() {
            return Ok(false);
        }
        git(&self.dir, &["checkout", "-q", "-B", branch])?;
        // the record says who did it: devlab, by itself
        let ok = |c: &mut Command| {
            c.env("GIT_AUTHOR_NAME", "devlab")
                .env("GIT_COMMITTER_NAME", "devlab")
                .env("GIT_AUTHOR_EMAIL", "agent@creation-guidelines")
                .env("GIT_COMMITTER_EMAIL", "agent@creation-guidelines")
                .env("OCOECO_BY", "devlab")
                .current_dir(&self.dir)
                .status()
                .is_ok_and(|s| s.success())
        };
        if !ok(Command::new("ocoeco").args(["glimpse", summary, "--kind", "docs"]))
            || !ok(Command::new("ocoeco").arg("commit"))
            || !ok(Command::new("land")
                .arg("--no-manual")
                .env("LAND_RELEASE", "0"))
        {
            // a landing that did not go through leaves nothing behind: its pull request is closed and
            // its branch deleted, so that the next try - from main as it then is - starts clean
            let _ = Command::new("gh")
                .args(["pr", "close", branch, "--delete-branch"])
                .current_dir(&self.dir)
                .output();
            let _ = git(&self.dir, &["checkout", "-q", "-f", "main"]);
            bail!(
                "landing {} did not go through - see above",
                self.dir.display()
            );
        }
        let _ = git(&self.dir, &["checkout", "-q", "-f", "main"]);
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_phones_run_reports_are_brought() {
        assert!(is_report("20261116-232731.jsonl"));
        assert!(!is_report("latest.jsonl"));
        assert!(!is_report("20261116-232731.evidence"));
        assert!(!is_report(".20261116-232731.jsonl.part"));
    }

    #[test]
    fn the_picture_says_what_it_derives() {
        let d = std::env::temp_dir().join(format!("devlab-derive-{}", std::process::id()));
        std::fs::create_dir_all(d.join("derive")).unwrap();
        for f in ["b.sql", "a.sql", "notes.md"] {
            std::fs::write(d.join("derive").join(f), "").unwrap();
        }
        let names: Vec<String> = derivations(&d)
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.sql", "b.sql"]);
        assert!(
            derivations(&d.join("none")).is_empty(),
            "a picture with nothing to derive"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn a_run_is_filed_as_the_report_says_it_and_counted_from_its_rows() {
        let s = statement(
            "20261117-010000",
            Path::new("/r/it's.jsonl"),
            &["camera-rear".to_string(), "o'gap".to_string()],
        );
        assert!(
            s.contains("count_if(outcome = 'fail' AND list_contains(['camera-rear', 'o''gap']::VARCHAR[], component_id))"),
            "a failure in a known gap is counted apart"
        );
        assert!(s.contains("'20261117-010000-' || component || '-' || test"));
        assert!(
            s.contains("read_json_auto('/r/it''s.jsonl'"),
            "a quote in a path stays a quote"
        );
        assert!(s.contains("WHERE run = '20261117-010000' GROUP BY run"));
    }
}
