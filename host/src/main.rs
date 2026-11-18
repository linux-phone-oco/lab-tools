//! lpoco-host: the lab host prepared from declarations (run as `lpoco host ...`). `plan` (the default) changes
//! nothing; `prepare now` changes only what differs; `check` reads. See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems, Resolution};
use cli::{Command, Contract};
use lpoco_host::*;
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

// Linux open flags: a tty without carrier must not block the check, nor become its controlling terminal
const O_NONBLOCK: i32 = 0o4000;
const O_NOCTTY: i32 = 0o400;
use std::path::{Path, PathBuf};
use std::process::Command as Proc;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage("lpoco-host", "The lab host prepared from declarations (run as lpoco host). Commands: plan (the default; changes nothing on the host, only fetches each device clone), prepare now (changes only what differs), check (reads)."));
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-host {VERSION}");
        return;
    }
    let code = match run(&r) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("lpoco-host: {e}");
            2
        }
    };
    std::process::exit(code);
}

/// Everything the verbs need, read once.
struct State {
    home: String,
    path: String,
    root: Vec<String>,
    devices: Vec<Device>,
    missed: Vec<String>,
    reads: Vec<String>,
    lab: Lab,
    fs_root: String,
}

fn load(r: &Resolution) -> Result<State, String> {
    let home = Contract::HOME.get_from(r).ok_or("HOME is not set")?;
    let lab_path = Contract::LAB
        .get_from(r)
        .unwrap_or_else(|| expand_home("~/Projects/lab-machine/lab.toml", &home));
    let lab = read_lab(Path::new(&lab_path))?;
    let root = Contract::ROOT
        .get_from(r)
        .or_else(|| Contract::DEVLAB_ROOT.get_from(r))
        .or_else(|| lab.root.clone())
        .unwrap_or_else(|| DEFAULT_ROOT.to_string());
    let patterns = lab
        .devices
        .as_ref()
        .map(Devices::patterns)
        .unwrap_or_else(|| vec![DEFAULT_DEVICES.to_string()]);
    let mut devices = Vec::new();
    let mut reads = Vec::new();
    for p in find_devices(&patterns, &home)? {
        let (text, how) = device_text(&p)?;
        reads.extend(how);
        devices.push(parse_device(&p, &text)?);
    }
    let (devices, mut notes) = one_per_codename(devices);
    let mut devices = devices;
    let misses = pin_misses(&lab, &devices);
    let mut missed: Vec<String> = misses.iter().map(|(_, m)| m.clone()).collect();
    missed.append(&mut notes);
    let skip: Vec<PathBuf> = misses.iter().map(|(d, _)| d.path.clone()).collect();
    devices.retain(|d| !skip.contains(&d.path));
    Ok(State {
        path: Contract::PATH.get_from(r).unwrap_or_default(),
        fs_root: Contract::FS_ROOT.get_from(r).unwrap_or_default(),
        home,
        root: root.split_whitespace().map(String::from).collect(),
        devices,
        missed,
        reads,
        lab,
    })
}

/// A device declaration as its repository says it: a clone is fetched (never prompting, bounded) and `device.toml` is
/// read from `origin/main`, so a stale checkout cannot hide a `[host]` table. The second part names what was read: the
/// commit, how far the checkout is behind it, whether the working file differs; or why the file as it lies was read.
/// A plain file outside any clone has nothing to say.
fn device_text(p: &Path) -> Result<(String, Option<String>), String> {
    let file = || fs::read_to_string(p).map_err(|e| format!("cannot read {}: {e}", p.display()));
    let lie = |why: &str| -> Result<(String, Option<String>), String> {
        Ok((
            file()?,
            Some(format!("{}: the file as it lies ({why})", p.display())),
        ))
    };
    let Some(dir) = p.parent() else {
        return Ok((file()?, None));
    };
    let git = |args: &[&str]| -> Result<String, String> {
        let o = Proc::new("git")
            .arg("-C")
            .arg(dir)
            .args([
                "-c",
                "http.lowSpeedLimit=1000",
                "-c",
                "http.lowSpeedTime=15",
            ])
            .args(args)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env(
                "GIT_SSH_COMMAND",
                "ssh -o BatchMode=yes -o ConnectTimeout=10",
            )
            .output()
            .map_err(|e| e.to_string())?;
        if o.status.success() {
            Ok(String::from_utf8_lossy(&o.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
        }
    };
    let top = match git(&["rev-parse", "--show-toplevel"]) {
        Ok(t) if !t.trim().is_empty() => PathBuf::from(t.trim()),
        Ok(_) => return Ok((file()?, None)),
        Err(e) if e.contains("not a git repository") => return Ok((file()?, None)),
        Err(e) => return lie(&format!("git refused: {e}")),
    };
    let Some(rel) = p
        .canonicalize()
        .ok()
        .and_then(|c| c.strip_prefix(&top).ok().map(Path::to_path_buf))
    else {
        return lie("it lies outside its clone");
    };
    let fetch = git(&["fetch", "--quiet", "origin"]);
    let stale = match &fetch {
        Ok(_) => String::new(),
        Err(e) => format!("; fetch failed, origin/main as last fetched: {e}"),
    };
    if git(&["rev-parse", "--verify", "--quiet", "origin/main"]).is_err() {
        return lie(&format!("no origin/main{stale}"));
    }
    let Ok(text) = git(&["show", &format!("origin/main:{}", rel.display())]) else {
        return lie(&format!("not in origin/main{stale}"));
    };
    let sha = git(&["rev-parse", "--short", "origin/main"]).unwrap_or_default();
    let behind = match git(&["rev-list", "--count", "HEAD..origin/main"]) {
        Ok(n) if !n.trim().is_empty() => n.trim().to_string(),
        _ => "unknown".to_string(),
    };
    let differs = match fs::read_to_string(p) {
        Ok(w) if w != text => "; the working file differs and is ignored",
        _ => "",
    };
    Ok((
        text,
        Some(format!(
            "{} from origin/main {}, checkout {behind} behind{differs}{stale}",
            p.display(),
            sha.trim()
        )),
    ))
}

fn on_path(path: &str, name: &str) -> Option<PathBuf> {
    path.split(':')
        .filter(|d| !d.is_empty())
        .map(|d| Path::new(d).join(name))
        .find(|p| {
            p.metadata()
                .is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        })
}

/// The version `NAME --version` reports: the last word of its first line.
fn version_found(path: &str, home: &str, name: &str) -> Option<String> {
    // the host's own ~/.local/bin counts even when PATH does not name it yet
    let local = format!("{home}/.local/bin");
    let prog = on_path(&format!("{local}:{path}"), name)?;
    let o = Proc::new(prog).arg("--version").output().ok()?;
    let line = String::from_utf8_lossy(&o.stdout)
        .lines()
        .next()?
        .to_string();
    line.split_whitespace().last().map(str::to_string)
}

fn kernel_modules_present(fs_root: &str) -> bool {
    if !fs_root.is_empty() {
        return true;
    }
    fs::read_to_string("/proc/sys/kernel/osrelease")
        .map(|r| Path::new("/lib/modules").join(r.trim()).is_dir())
        .unwrap_or(true)
}

fn module_loaded(fs_root: &str, m: &str) -> bool {
    if !fs_root.is_empty() {
        return false;
    }
    Path::new("/sys/module").join(m.replace('-', "_")).exists()
}

/// The root commands that make `files` real, only where they differ: the fixed list the plan prints.
fn root_commands(
    files: &[FileChange],
    modules: &[String],
    stage: &str,
    fs_root: &str,
) -> Vec<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut cmds = Vec::new();
    let mut rules_changed = false;
    let mut serials = Vec::new();
    for f in files.iter().filter(|f| f.changed()) {
        let base = f.dst.rsplit('/').next().unwrap_or(&f.dst);
        cmds.push(s(&[
            "install",
            "-D",
            "-m",
            "0644",
            &format!("{stage}/{base}"),
            &f.dst,
        ]));
        if f.dst.starts_with(UDEV_DIR) {
            rules_changed = true;
            serials.extend(f.serials.clone());
        }
    }
    if rules_changed {
        cmds.push(s(&["udevadm", "control", "--reload"]));
        for sn in serials {
            cmds.push(s(&[
                "udevadm",
                "trigger",
                "--action=change",
                &format!("--attr-match=serial={sn}"),
            ]));
        }
        // the rules match the block and tty children of the USB device, which --attr-match on the device itself does not reach
        cmds.push(s(&[
            "udevadm",
            "trigger",
            "--action=change",
            "--subsystem-match=block",
            "--subsystem-match=tty",
        ]));
    }
    let modules_file_changed = files.iter().any(|f| f.dst == MODULES_FILE && f.changed());
    if kernel_modules_present(fs_root) {
        for m in modules
            .iter()
            .filter(|m| modules_file_changed || !module_loaded(fs_root, m))
        {
            cmds.push(s(&["modprobe", m]));
        }
    }
    cmds
}

fn stage_dir(home: &str) -> String {
    format!("{home}/.cache/lpoco-host/stage")
}

fn run(r: &Resolution) -> Result<i32, String> {
    let command = Contract::COMMAND.get_from(r).unwrap_or(Command::Plan);
    let st = load(r)?;
    match command {
        Command::Plan => {
            plan(&st);
            Ok(0)
        }
        Command::Prepare => {
            if Contract::NOW.get_from(r).as_deref() != Some("now") {
                return Err("prepare changes the host: say `lpoco host prepare now` (plain `lpoco host` shows the plan)".into());
            }
            prepare(&st)
        }
        Command::Check => check(&st),
    }
}

fn read_file(fs_root: &str, p: &str) -> Option<String> {
    fs::read_to_string(format!("{fs_root}{p}")).ok()
}

fn tool_plans(st: &State) -> Vec<ToolPlan> {
    tools_for(
        &st.lab,
        &st.home,
        &|n| version_found(&st.path, &st.home, n),
        &|n| {
            fs::read_to_string(stamp_path(&st.home, n))
                .ok()
                .map(|s| s.trim().to_string())
        },
        &|n| on_path(&format!("{}/.local/bin:{}", st.home, st.path), n).is_some(),
    )
}

fn plan(st: &State) {
    let files = files_for(&st.devices, &|p| read_file(&st.fs_root, p));
    let modules = modules_of(&st.devices);
    let stage = stage_dir(&st.home);
    println!("lpoco host plan (nothing on the host is changed; each device clone is fetched)");
    for r in &st.reads {
        println!("read: {r}");
    }
    for m in &st.missed {
        println!("not prepared: {m}");
    }
    println!(
        "\ndevices: {}",
        if st.devices.is_empty() {
            "none declare a [host] table".into()
        } else {
            st.devices
                .iter()
                .map(|d| d.codename.clone())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    for f in &files {
        if f.changed() {
            println!(
                "\n{} ({})",
                f.dst,
                if f.existing.is_some() {
                    "changes"
                } else {
                    "new"
                }
            );
            for l in f.diff() {
                println!("    {l}");
            }
        } else {
            println!("\n{} (as declared)", f.dst);
        }
    }
    if modules.iter().any(|m| !module_loaded(&st.fs_root, m))
        && !kernel_modules_present(&st.fs_root)
    {
        println!("\nreboot: the running kernel has no modules (/lib/modules for it is gone), so {} cannot be loaded until the host boots its installed kernel", modules.join(", "));
    }
    let cmds = root_commands(&files, &modules, &stage, &st.fs_root);
    println!("\nroot commands, through `{}`:", st.root.join(" "));
    if cmds.is_empty() {
        println!("    none");
    }
    for c in &cmds {
        println!("    {}", c.join(" "));
    }
    if Path::new(&format!("{}{KEPT_RULE}", st.fs_root)).exists() {
        println!("\nkept: {KEPT_RULE} is a wide rule; it is kept for now and prepare never removes or edits it");
    }
    println!("\ntools:");
    for t in tool_plans(st) {
        let found = t.found_text();
        if t.unpinned() {
            println!(
                "    {:<14} pin not known yet, found {found}: nothing installed",
                t.name
            );
            continue;
        }
        if t.ok() {
            println!(
                "    {:<14} pinned {:<8} found {found}: nothing to do",
                t.name, t.pinned
            );
        } else {
            println!("    {:<14} pinned {:<8} found {found}", t.name, t.pinned);
            for s in &t.steps {
                println!("        {}", s.show());
            }
            if let Some(by) = &t.installed_by {
                println!("        off the pin: an older {} is first on PATH; {by} installs this name too, so reinstall {by} into ~/.local and remove any copy that shadows it", t.name);
            }
        }
    }
}

fn run_root(st: &State, cmd: &[String]) -> Result<(), String> {
    let (prog, pre) = st.root.split_first().ok_or("the root prefix is empty")?;
    let status = Proc::new(prog)
        .args(pre)
        .args(cmd)
        .status()
        .map_err(|e| format!("cannot run {prog}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("root command failed ({status}): {}", cmd.join(" ")))
    }
}

fn prepare(st: &State) -> Result<i32, String> {
    for r in &st.reads {
        println!("read: {r}");
    }
    let files = files_for(&st.devices, &|p| read_file(&st.fs_root, p));
    let modules = modules_of(&st.devices);
    let stage = stage_dir(&st.home);
    // root installs from here: a fresh folder only this user can write
    let _ = fs::remove_dir_all(&stage);
    fs::create_dir_all(&stage).map_err(|e| format!("cannot make {stage}: {e}"))?;
    fs::set_permissions(&stage, fs::Permissions::from_mode(0o700))
        .map_err(|e| format!("{stage}: {e}"))?;
    for f in files.iter().filter(|f| f.changed()) {
        let base = f.dst.rsplit('/').next().unwrap_or(&f.dst);
        fs::write(format!("{stage}/{base}"), &f.content)
            .map_err(|e| format!("cannot stage {base}: {e}"))?;
    }
    let cmds = root_commands(&files, &modules, &stage, &st.fs_root);
    for c in &cmds {
        println!("root: {}", c.join(" "));
        run_root(st, c)?;
    }
    let _ = fs::remove_dir_all(&stage);
    for t in tool_plans(st).into_iter().filter(|t| !t.ok()) {
        println!("tool {}: pinned {}", t.name, t.pinned);
        for s in &t.steps {
            match s {
                Step::Run(a) => {
                    if a.first().is_some_and(|x| x == "git")
                        && a.get(1).is_some_and(|x| x == "clone")
                    {
                        // a clone into a folder left by an earlier run: start it again
                        let _ = fs::remove_dir_all(a.last().unwrap());
                    }
                    let status = Proc::new(&a[0])
                        .args(&a[1..])
                        .status()
                        .map_err(|e| format!("cannot run {}: {e}", a[0]))?;
                    if !status.success() {
                        return Err(format!("{} failed ({status})", s.show()));
                    }
                }
                Step::Stamp { to, rev } => {
                    if let Some(d) = Path::new(to).parent() {
                        fs::create_dir_all(d)
                            .map_err(|e| format!("cannot make {}: {e}", d.display()))?;
                    }
                    fs::write(to, format!("{rev}\n"))
                        .map_err(|e| format!("cannot write {to}: {e}"))?;
                }
                Step::Copy { from, to } => {
                    if let Some(d) = Path::new(to).parent() {
                        fs::create_dir_all(d)
                            .map_err(|e| format!("cannot make {}: {e}", d.display()))?;
                    }
                    fs::copy(from, to).map_err(|e| format!("cannot copy {from} to {to}: {e}"))?;
                    fs::set_permissions(to, fs::Permissions::from_mode(0o755))
                        .map_err(|e| format!("{to}: {e}"))?;
                }
            }
        }
    }
    println!("prepared: {} root command(s)", cmds.len());
    Ok(if st.missed.is_empty() { 0 } else { 1 })
}

/// Device nodes of `class` (block or tty) whose USB ancestor reports `serial`.
fn nodes_with_serial(class: &str, serial: &str) -> Vec<String> {
    let mut found = Vec::new();
    let Ok(rd) = fs::read_dir(Path::new("/sys/class").join(class)) else {
        return found;
    };
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if class == "tty" && !(name.starts_with("ttyACM") || name.starts_with("ttyUSB")) {
            continue;
        }
        let Ok(real) = fs::canonicalize(e.path()) else {
            continue;
        };
        if real.join("partition").exists() {
            continue;
        }
        let mut dir = real.parent();
        while let Some(d) = dir {
            if let Ok(s) = fs::read_to_string(d.join("serial")) {
                if s.trim() == serial {
                    found.push(format!("/dev/{name}"));
                }
                break;
            }
            dir = d.parent();
        }
    }
    found.sort();
    found
}

fn check(st: &State) -> Result<i32, String> {
    for r in &st.reads {
        println!("read: {r}");
    }
    let mut bad = 0;
    if Path::new(&format!("{}{KEPT_RULE}", st.fs_root)).exists() {
        println!("kept: {KEPT_RULE} is a wide rule, kept for now");
    }
    for d in &st.devices {
        for g in &d.host.gadget {
            for a in &g.access {
                let nodes = nodes_with_serial(a.subsystem(), &g.serial);
                if nodes.is_empty() {
                    println!(
                        "{} {} {}: not measured, no such node present",
                        d.codename,
                        a.subsystem(),
                        g.serial
                    );
                }
                for n in nodes {
                    let mut o = fs::OpenOptions::new();
                    o.read(true)
                        .write(*a == Access::Tty)
                        .custom_flags(O_NONBLOCK | O_NOCTTY);
                    match o.open(&n) {
                        Ok(_) => println!(
                            "{} {}: ok, {n} opens as this user",
                            d.codename,
                            a.subsystem()
                        ),
                        Err(e) => {
                            bad += 1;
                            println!(
                                "{} {}: FAIL, {n} does not open as this user: {e}",
                                d.codename,
                                a.subsystem()
                            );
                        }
                    }
                }
            }
        }
        if let Some(s) = &d.fastboot_serial {
            match Proc::new("fastboot").arg("devices").output() {
                Ok(o) if String::from_utf8_lossy(&o.stdout).contains(s.as_str()) => {
                    println!("{} fastboot: ok, serial {s} listed", d.codename)
                }
                Ok(_) => println!(
                    "{} fastboot: not measured, serial {s} is not in fastboot now",
                    d.codename
                ),
                Err(_) => println!("{} fastboot: not measured, no fastboot on PATH", d.codename),
            }
        }
    }
    for t in tool_plans(st) {
        if t.unpinned() {
            println!("tool {}: not measured, its pin is not known yet", t.name);
        } else if t.ok() {
            println!("tool {}: ok, {}", t.name, t.found.unwrap_or_default());
        } else {
            bad += 1;
            println!(
                "tool {}: FAIL, pinned {} found {}",
                t.name,
                t.pinned,
                t.found_text()
            );
        }
    }
    for m in &st.missed {
        bad += 1;
        println!("FAIL {m}");
    }
    Ok(if bad == 0 { 0 } else { 1 })
}
