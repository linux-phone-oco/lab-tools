//! lpoco-lab (devlab): the device lab for Linux phones - for any device, from its manifest (device.toml).
//!
//!   devlab phone find              which address answers
//!   devlab phone sh   < script     run commands as the user (script on stdin)
//!   devlab phone root < script     run commands as root
//!   devlab phone put LOCAL REMOTE  copy a file to the phone
//!   devlab phone reboot            reboot and wait until it answers again
//!   devlab kernel install NAME VMLINUZ DTB MODULES.tgz | try NAME | log | remove NAME | keep|unkeep NAME now
//!   devlab test                    the device's checks, now
//!   devlab hear                    a tone on each speaker, heard through the lab's microphone
//!   devlab health                  against everything that worked before; exit 1 on a regression
//!   devlab suite put now [--runner FILE | --built-runner] | run | latest   the health suite on the phone itself
//!   devlab gather now [--land] [--docs DIR]  the phone's reports into this lab, their runs into its base (docs with --docs)
//!   devlab mcp                    the phone for any agent: an MCP server (stdio)
//!   devlab fastboot state | getvar NAME | boot now IMAGE | reboot now | set-active now SLOT   by serial
//!   devlab keeper take|renew|release NAME | status   the phone's keeper, one at a time
//!   devlab guard tool              the phone's way, held for agents: no ssh/scp to a phone outside devlab (a hook)
//!   devlab watch                   after every boot: health, silent when well, a board task when not
//!                                  a test kernel beside stock, tried once, never stranding the phone

mod cli; // the command line, declared once with argenv
mod device;
mod fastboot;
mod flash;
mod gather;
mod guard;
mod health;
mod hear;
mod hold;
mod keeper;
mod kernel;
mod mcp;
mod outcome;
mod phone;
mod provision;
mod suite;
mod watch;

use anyhow::{bail, Result};
use std::io::Read;
use std::path::PathBuf;

struct Cli {
    device: Option<PathBuf>,
    cmd: Cmd,
}

enum Cmd {
    Phone {
        action: PhoneCmd,
    },
    Flash {
        now: String,
        image: PathBuf,
        bootloader: PathBuf,
        dtb: Option<PathBuf>,
        as_is: bool,
    },
    Provision {
        now: String,
    },
    Test,
    Hear {
        placed: bool,
    },
    Health {
        hear: bool,
        placed: bool,
    },
    Gather {
        now: String,
        docs: Option<PathBuf>,
        land: bool,
    },
    Suite {
        action: SuiteCmd,
    },
    Mcp,
    Watch,
    Fastboot {
        action: FastbootCmd,
    },
    Keeper {
        action: KeeperCmd,
    },
    Guard {
        event: GuardCmd,
    },
    Kernel {
        action: KernelCmd,
    },
}

enum KeeperCmd {
    Take { name: String },
    Renew { name: String },
    Release { name: String },
    Status,
}

enum GuardCmd {
    Tool,
}

enum KernelCmd {
    Install {
        name: String,
        vmlinuz: PathBuf,
        dtb: PathBuf,
        modules: PathBuf,
        cmdline: String,
    },
    Try {
        name: String,
        dtb: Option<PathBuf>,
    },
    Log,
    Remove {
        name: String,
    },
    Keep {
        name: String,
        now: String,
    },
    Unkeep {
        name: String,
        now: String,
    },
}

enum FastbootCmd {
    State,
    Getvar { name: String },
    Boot { now: String, image: PathBuf },
    Reboot { now: String, bootloader: bool },
    SetActive { now: String, slot: String },
}

enum SuiteCmd {
    Put {
        now: String,
        runner: Option<Option<PathBuf>>,
        here: bool,
    },
    Run,
    Latest,
    Off {
        test: String,
        reason: String,
        by: String,
    },
    On {
        test: String,
    },
    List,
}

enum PhoneCmd {
    Find,
    Sh,
    Root,
    Put { local: String, remote: String },
    Reboot,
}

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A usage mistake: said with the usage of what was meant, exit 2.
fn usage_error(found: &cli::Found, said: &str) -> ! {
    eprintln!("lpoco-lab: {said}\n\n{}", cli::usage(found));
    std::process::exit(2);
}

/// The command line, read once through its argenv declaration (cli.rs).
fn parse() -> Cli {
    use argenv_resolve::InputResolveExt;
    use cli::Contract as C;
    let model = C::records();
    let r = argenv_resolve::parse(&model);
    if C::VERSION.get_from(&r).unwrap_or(false) {
        println!("lpoco-lab {VERSION}");
        std::process::exit(0);
    }
    let words = r.positionals().to_vec();
    let found = cli::find(&words);
    if C::HELP.get_from(&r).unwrap_or(false) {
        print!("{}", cli::usage(&found));
        std::process::exit(0);
    }
    // a mistake in the words is a usage mistake: exit 2 with the usage, as clap had it (exit 1 stays for what a command itself says)
    let policy = argenv_resolve::LintPolicy::default();
    let errors: Vec<String> = argenv_resolve::lint(&model, &r)
        .iter()
        .filter(|f| policy.severity(f) == argenv_resolve::Severity::Error)
        .map(|f| f.to_string())
        .collect();
    if !errors.is_empty() {
        usage_error(&found, &errors.join("\n"));
    }
    match cli::env_in(&r) {
        Ok(e) => cli::set_env(e),
        Err(said) => usage_error(&found, &said),
    }
    let node = match &found {
        cli::Found::Leaf(n) => *n,
        cli::Found::Unknown(w) => usage_error(&found, &format!("unrecognized command '{w}'")),
        cli::Found::Group(_) => usage_error(&found, "an action is needed"),
        cli::Found::Top => usage_error(&found, "a command is needed"),
    };
    // flags are scoped by argenv to the command; the action it was given to is checked here, from the tree
    for (key, v) in r.values() {
        let given = v.source == argenv_resolve::Source::Arg;
        let is_flag = model
            .iter()
            .any(|m| m.key == *key && m.arg.as_ref().is_some_and(|a| a.position.is_none()));
        if given
            && is_flag
            && !node.flags.contains(&key.as_str())
            && !["device", "help", "version"].contains(&key.as_str())
        {
            usage_error(
                &found,
                &format!(
                    "--{} does not apply to {}",
                    key.replace('_', "-"),
                    node.path.join(" ")
                ),
            );
        }
    }
    if words.len() > node.path.len() + node.args.len() {
        usage_error(
            &found,
            &format!(
                "unexpected argument '{}'",
                words[node.path.len() + node.args.len()]
            ),
        );
    }
    // each argument the command takes is required: say which is missing
    macro_rules! arg {
        ($i:expr, $name:literal) => {
            match $i.get_from(&r) {
                Some(v) => v,
                None => usage_error(&found, concat!("the argument ", $name, " is needed")),
            }
        };
    }
    let flag = |b: Option<bool>| b.unwrap_or(false);
    let cmd = match node.path {
        ["phone", "find"] => Cmd::Phone {
            action: PhoneCmd::Find,
        },
        ["phone", "sh"] => Cmd::Phone {
            action: PhoneCmd::Sh,
        },
        ["phone", "root"] => Cmd::Phone {
            action: PhoneCmd::Root,
        },
        ["phone", "put"] => Cmd::Phone {
            action: PhoneCmd::Put {
                local: arg!(C::LOCAL, "LOCAL"),
                remote: arg!(C::REMOTE, "REMOTE"),
            },
        },
        ["phone", "reboot"] => Cmd::Phone {
            action: PhoneCmd::Reboot,
        },
        ["flash"] => Cmd::Flash {
            now: arg!(C::NOW, "NOW"),
            image: arg!(C::IMAGE, "IMAGE").into(),
            bootloader: arg!(C::BOOTLOADER_FILE, "BOOTLOADER").into(),
            dtb: C::DTB.get_from(&r).map(PathBuf::from),
            as_is: flag(C::AS_IS.get_from(&r)),
        },
        ["provision"] => Cmd::Provision {
            now: arg!(C::NOW, "NOW"),
        },
        ["test"] => Cmd::Test,
        ["hear"] => Cmd::Hear {
            placed: flag(C::PLACED.get_from(&r)),
        },
        ["health"] => Cmd::Health {
            hear: flag(C::HEAR.get_from(&r)),
            placed: flag(C::PLACED.get_from(&r)),
        },
        ["gather"] => Cmd::Gather {
            now: arg!(C::NOW, "NOW"),
            docs: C::DOCS.get_from(&r).map(PathBuf::from),
            land: flag(C::LAND.get_from(&r)),
        },
        ["suite", "put"] => {
            let (file, built) = (C::RUNNER.get_from(&r), flag(C::BUILT_RUNNER.get_from(&r)));
            if file.is_some() && built {
                usage_error(
                    &found,
                    "--runner FILE and --built-runner are two ways to say which runner: say one",
                );
            }
            Cmd::Suite {
                action: SuiteCmd::Put {
                    now: arg!(C::ACTION_NOW, "NOW"),
                    runner: match (file, built) {
                        (Some(f), _) => Some(Some(PathBuf::from(f))),
                        (None, true) => Some(None),
                        (None, false) => None,
                    },
                    here: flag(C::HERE.get_from(&r)),
                },
            }
        }
        ["suite", "run"] => Cmd::Suite {
            action: SuiteCmd::Run,
        },
        ["suite", "latest"] => Cmd::Suite {
            action: SuiteCmd::Latest,
        },
        ["suite", "off"] => Cmd::Suite {
            action: SuiteCmd::Off {
                test: arg!(C::TEST, "TEST"),
                reason: match C::REASON.get_from(&r) {
                    Some(v) => v,
                    None => usage_error(
                        &found,
                        "--reason is needed: a test is never switched off without one",
                    ),
                },
                by: C::BY.get_from(&r).unwrap_or_else(|| "devlab".into()),
            },
        },
        ["suite", "on"] => Cmd::Suite {
            action: SuiteCmd::On {
                test: arg!(C::TEST, "TEST"),
            },
        },
        ["suite", "list"] => Cmd::Suite {
            action: SuiteCmd::List,
        },
        ["mcp"] => Cmd::Mcp,
        ["watch"] => Cmd::Watch,
        ["fastboot", "state"] => Cmd::Fastboot {
            action: FastbootCmd::State,
        },
        ["fastboot", "getvar"] => Cmd::Fastboot {
            action: FastbootCmd::Getvar {
                name: arg!(C::NAME, "NAME"),
            },
        },
        ["fastboot", "boot"] => Cmd::Fastboot {
            action: FastbootCmd::Boot {
                now: arg!(C::ACTION_NOW, "NOW"),
                image: arg!(C::FB_IMAGE, "IMAGE").into(),
            },
        },
        ["fastboot", "reboot"] => Cmd::Fastboot {
            action: FastbootCmd::Reboot {
                now: arg!(C::ACTION_NOW, "NOW"),
                bootloader: flag(C::BOOTLOADER.get_from(&r)),
            },
        },
        ["fastboot", "set-active"] => Cmd::Fastboot {
            action: FastbootCmd::SetActive {
                now: arg!(C::ACTION_NOW, "NOW"),
                slot: arg!(C::SLOT, "SLOT"),
            },
        },
        ["keeper", "take"] => Cmd::Keeper {
            action: KeeperCmd::Take {
                name: arg!(C::NAME, "NAME"),
            },
        },
        ["keeper", "renew"] => Cmd::Keeper {
            action: KeeperCmd::Renew {
                name: arg!(C::NAME, "NAME"),
            },
        },
        ["keeper", "release"] => Cmd::Keeper {
            action: KeeperCmd::Release {
                name: arg!(C::NAME, "NAME"),
            },
        },
        ["keeper", "status"] => Cmd::Keeper {
            action: KeeperCmd::Status,
        },
        ["guard", "tool"] => Cmd::Guard {
            event: GuardCmd::Tool,
        },
        ["kernel", "install"] => Cmd::Kernel {
            action: KernelCmd::Install {
                name: arg!(C::NAME, "NAME"),
                vmlinuz: arg!(C::VMLINUZ, "VMLINUZ").into(),
                dtb: arg!(C::DTB_FILE, "DTB").into(),
                modules: arg!(C::MODULES, "MODULES").into(),
                cmdline: C::CMDLINE.get_from(&r).unwrap_or_default(),
            },
        },
        ["kernel", "try"] => Cmd::Kernel {
            action: KernelCmd::Try {
                name: arg!(C::NAME, "NAME"),
                dtb: C::DTB.get_from(&r).map(PathBuf::from),
            },
        },
        ["kernel", "log"] => Cmd::Kernel {
            action: KernelCmd::Log,
        },
        ["kernel", "remove"] => Cmd::Kernel {
            action: KernelCmd::Remove {
                name: arg!(C::NAME, "NAME"),
            },
        },
        ["kernel", "keep"] => Cmd::Kernel {
            action: KernelCmd::Keep {
                name: arg!(C::NAME, "NAME"),
                now: arg!(C::KERNEL_NOW, "NOW"),
            },
        },
        ["kernel", "unkeep"] => Cmd::Kernel {
            action: KernelCmd::Unkeep {
                name: arg!(C::NAME, "NAME"),
                now: arg!(C::KERNEL_NOW, "NOW"),
            },
        },
        other => unreachable!("cli::TREE has {other:?} but parse() does not"),
    };
    Cli {
        device: C::DEVICE.get_from(&r).map(PathBuf::from),
        cmd,
    }
}
fn main() -> Result<()> {
    let cli = parse();
    if let Cmd::Guard { event } = &cli.cmd {
        return match event {
            GuardCmd::Tool => guard::tool(),
        };
    }
    let (dev, file) = device::Device::load(cli.device.as_deref(), &std::env::current_dir()?)?;
    dev.remember_addresses();
    // one change to the phone at a time, whoever asks: held until this command ends
    let _held = match &cli.cmd {
        Cmd::Flash { .. } | Cmd::Provision { .. } => Some(hold::hold(&dev)?),
        Cmd::Phone {
            action: PhoneCmd::Reboot | PhoneCmd::Root | PhoneCmd::Put { .. },
        } => Some(hold::hold(&dev)?),
        Cmd::Kernel {
            action:
                KernelCmd::Install { .. }
                | KernelCmd::Try { .. }
                | KernelCmd::Remove { .. }
                | KernelCmd::Keep { .. }
                | KernelCmd::Unkeep { .. },
        } => Some(hold::hold(&dev)?),
        Cmd::Suite {
            action: SuiteCmd::Put { .. },
        } => Some(hold::hold(&dev)?),
        _ => None,
    };
    let here = file
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_default();
    match cli.cmd {
        Cmd::Flash {
            now,
            image,
            bootloader,
            dtb,
            as_is,
        } => {
            if now != "now" {
                bail!("usage: lpoco-lab flash now IMAGE BOOTLOADER - it overwrites the phone");
            }
            flash::flash(&dev, &here, &image, &bootloader, dtb.as_deref(), as_is)?;
        }
        Cmd::Provision { now } => {
            if now != "now" {
                bail!("usage: lpoco-lab provision now - it shares this machine's connection with the phone for a while");
            }
            provision::provision(&dev, &here)?;
            let (well, said) = suite::after_change(&phone::Phone::find(&dev)?, false)?;
            println!("{said}");
            if !well {
                std::process::exit(1);
            }
        }
        Cmd::Hear { placed } => {
            hear::placed_or_refuse(placed)?;
            let checks = hear::hear(&dev, &phone::Phone::find(&dev)?)?;
            for c in &checks {
                println!("{:<5} {}  {}", c.outcome, c.key, c.detail);
            }
            if checks.iter().any(|c| c.outcome != "pass") {
                bail!("not every tone was heard");
            }
        }
        Cmd::Test => {
            let checks = health::run_checks(&dev, &here, &phone::Phone::find(&dev)?)?;
            for c in &checks {
                println!("{:5} {} - {}", c.outcome, c.key, c.detail);
            }
            let n = |o: &str| checks.iter().filter(|c| c.outcome == o).count();
            println!("{} pass, {} fail, {} skip", n("pass"), n("fail"), n("skip"));
        }
        Cmd::Health { hear, placed } => {
            if hear {
                hear::placed_or_refuse(placed)?;
            }
            let r = health::health(&dev, &here, &phone::Phone::find(&dev)?, hear)?;
            if r.is_empty() {
                println!(
                    "all is well ({} checks in the baseline)",
                    health::load_baseline(&dev).len()
                );
            } else {
                for x in &r {
                    println!("regressed: {x}");
                }
                std::process::exit(1);
            }
        }
        Cmd::Gather { now, docs, land } => {
            if now != "now" {
                bail!("usage: lpoco-lab gather now [--docs DIR] [--land] - it writes into this lab (and docs without a base)");
            }
            println!("{}", gather::gather(&dev, &here, docs, land)?);
        }
        Cmd::Suite { action } => {
            let p = phone::Phone::find(&dev)?;
            match action {
                SuiteCmd::Put {
                    now,
                    runner,
                    here: as_is,
                } => {
                    if now != "now" {
                        bail!("usage: lpoco-lab suite put now [--runner FILE | --built-runner] [--here] - it changes the phone");
                    }
                    let runner = match runner {
                        Some(None) => Some(suite::built_runner()?),
                        Some(Some(file)) => Some(file),
                        None => None,
                    };
                    if as_is {
                        suite::put(&dev, &here, &p, runner.as_deref())?;
                    } else {
                        // what the lab's main holds - never a clone that has fallen behind (2026-11-17:
                        // a clone 40 commits behind put back checks and a gap main had left)
                        let lab =
                            gather::Place::at_main(&here, &dev.state_dir().join("places-put"))?;
                        let (main_dev, _) =
                            device::Device::load(Some(&lab.dir.join(device::MANIFEST)), &lab.dir)?;
                        eprintln!("the lab as its main has it: {}", lab.dir.display());
                        suite::put(&main_dev, &lab.dir, &p, runner.as_deref())?;
                    }
                }
                SuiteCmd::Run => {
                    let ran = suite::run(&p)?;
                    print!("{}", suite::said(&suite::rows(&ran.report)));
                    println!("the phone says: {}", ran.verdict);
                    if !ran.well {
                        std::process::exit(1);
                    }
                }
                SuiteCmd::Latest => print!("{}", suite::latest(&p)?),
                SuiteCmd::Off { test, reason, by } => {
                    print!("{}", suite::off(&p, &test, &reason, &by)?)
                }
                SuiteCmd::On { test } => print!("{}", suite::on(&p, &test)?),
                SuiteCmd::List => print!("{}", suite::list(&p)?),
            }
        }
        Cmd::Mcp => mcp::serve(dev)?,
        Cmd::Watch => watch::watch(&dev, &here)?,
        Cmd::Fastboot { action } => {
            match action {
                FastbootCmd::State => print!("{}", fastboot::state(&dev)?),
                FastbootCmd::Getvar { name } => println!("{}", fastboot::getvar(&dev, &name)?),
                FastbootCmd::Boot { now, image } => {
                    if now != "now" {
                        bail!("usage: lpoco-lab fastboot boot now IMAGE - it boots the phone");
                    }
                    println!("{}", fastboot::boot(&dev, &image)?);
                }
                FastbootCmd::Reboot { now, bootloader } => {
                    if now != "now" {
                        bail!("usage: lpoco-lab fastboot reboot now [--bootloader] - it reboots the phone");
                    }
                    println!("{}", fastboot::reboot(&dev, bootloader)?);
                }
                FastbootCmd::SetActive { now, slot } => {
                    if now != "now" {
                        bail!("usage: lpoco-lab fastboot set-active now SLOT - it changes the phone's active slot");
                    }
                    println!("{}", fastboot::set_active(&dev, &slot)?);
                }
            }
        }
        Cmd::Keeper { action } => match action {
            KeeperCmd::Take { name } => println!("{}", keeper::take(&dev, &name)?),
            KeeperCmd::Renew { name } => println!("{}", keeper::renew(&dev, &name)?),
            KeeperCmd::Release { name } => println!("{}", keeper::release(&dev, &name)?),
            KeeperCmd::Status => println!("{}", keeper::status(&dev)),
        },
        Cmd::Guard { .. } => unreachable!("answered before the manifest is read"),
        Cmd::Kernel { action } => {
            let p = phone::Phone::find(&dev)?;
            match action {
                KernelCmd::Install {
                    name,
                    vmlinuz,
                    dtb,
                    modules,
                    cmdline,
                } => kernel::install(&p, &name, &vmlinuz, &dtb, &modules, &cmdline)?,
                KernelCmd::Try { name, dtb } => {
                    let name = match dtb {
                        Some(dtb) => kernel::with_dtb(&p, &name, &dtb)?,
                        None => name,
                    };
                    println!("{}", outcome::try_and_report(&dev, &p, &name)?);
                    // the boot the try came to - the test kernel or stock - is measured like any other
                    let now = phone::Phone::find(&dev)?;
                    let (well, said) = suite::after_change(&now, true)?;
                    println!("{said}");
                    if !well {
                        std::process::exit(1);
                    }
                }
                KernelCmd::Log => kernel::log(&p)?,
                KernelCmd::Remove { name } => kernel::remove(&p, &name)?,
                KernelCmd::Keep { name, now } | KernelCmd::Unkeep { name, now } if now != "now" => {
                    bail!("usage: lpoco-lab kernel keep|unkeep {name} now - it changes which kernel the phone boots")
                }
                KernelCmd::Keep { name, .. } => kernel::keep(&p, &name)?,
                KernelCmd::Unkeep { name, .. } => kernel::unkeep(&p, &name)?,
            }
        }
        Cmd::Phone { action } => {
            let p = phone::Phone::find(&dev)?;
            match action {
                PhoneCmd::Find => println!("{}", p.target),
                PhoneCmd::Sh | PhoneCmd::Root => {
                    let mut script = String::new();
                    std::io::stdin().read_to_string(&mut script)?;
                    let (ok, out) = if matches!(action, PhoneCmd::Root) {
                        p.root(&script)?
                    } else {
                        p.sh(&script)?
                    };
                    print!("{out}");
                    if !ok {
                        std::process::exit(1);
                    }
                }
                PhoneCmd::Put { local, remote } => p.put(&local, &remote)?,
                PhoneCmd::Reboot => {
                    let back = phone::reboot(&dev, &p)?;
                    let up = back.uptime().unwrap_or(0);
                    if up == 0 {
                        bail!("back, but its uptime could not be read");
                    }
                    println!("back: {} (up {up}s)", back.target);
                    let (well, said) = suite::after_change(&back, true)?;
                    println!("{said}");
                    if !well {
                        std::process::exit(1);
                    }
                }
            }
        }
    }
    Ok(())
}
