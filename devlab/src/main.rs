//! devlab: the device lab for Linux phones - for any device, from its manifest (device.toml).
//!
//!   devlab phone find              which address answers
//!   devlab phone sh   < script     run commands as the user (script on stdin)
//!   devlab phone root < script     run commands as root
//!   devlab phone put LOCAL REMOTE  copy a file to the phone
//!   devlab phone reboot            reboot and wait until it answers again
//!   devlab kernel install NAME VMLINUZ DTB MODULES.tgz | try NAME | log | remove NAME
//!   devlab test                    the device's checks, now
//!   devlab hear                    a tone on each speaker, heard through the lab's microphone
//!   devlab health                  against everything that worked before; exit 1 on a regression
//!   devlab suite put now --runner F | run | latest   the health suite on the phone itself
//!   devlab gather now [--docs DIR]  the phone's reports into this lab and the picture's tables
//!   devlab mcp                    the phone for any agent: an MCP server (stdio)
//!   devlab guard tool | start      the ways, held for agents: what Claude Code's hooks on the lab call
//!   devlab watch                   after every boot: health, silent when well, a board task when not
//!                                  a test kernel beside stock, tried once, never stranding the phone

mod device;
mod flash;
mod gather;
mod guard;
mod health;
mod hear;
mod hold;
mod kernel;
mod mcp;
mod outcome;
mod phone;
mod provision;
mod suite;
mod watch;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::io::Read;
use std::path::PathBuf;

#[derive(Parser)]
#[command(
    version,
    about = "The device lab for Linux phones, for any device from its manifest"
)]
struct Cli {
    /// The device manifest (default: device.toml here or in a folder above)
    #[arg(long, global = true)]
    device: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Reach the phone
    Phone {
        #[command(subcommand)]
        action: PhoneCmd,
    },
    /// Flash an image (say `now`): the manifest's fixes baked in, the device tree fingerprinted
    Flash {
        now: String,
        image: PathBuf,
        bootloader: PathBuf,
        /// A device tree to put in the image's place for it
        #[arg(long)]
        dtb: Option<PathBuf>,
        /// Bake in nothing but access - to tell an image's fault from ours
        #[arg(long)]
        as_is: bool,
    },
    /// Provision a freshly flashed phone: packages, files, Wi-Fi (say `now`: it changes this machine's networking for a while)
    Provision { now: String },
    /// Run the device's checks now
    Test,
    /// Listen: play a tone on each speaker and hear it through the lab's microphone
    Hear {
        /// The word that the microphone has been placed by the phone (its keeper told, and agreed)
        #[arg(long)]
        placed: bool,
    },
    /// The checks against everything that worked before; exit 1 on a regression
    Health {
        /// Listen too: play a tone on each speaker and hear it through the lab's microphone
        #[arg(long)]
        hear: bool,
        /// With --hear: the word that the microphone has been placed by the phone
        #[arg(long)]
        placed: bool,
    },
    /// Gather (say `now`): every report the phone keeps that the lab lacks, into this lab's tests/results,
    /// and every run not yet filed, into the picture's test_runs and test_results
    Gather {
        now: String,
        /// The picture's place (default: docs beside this lab: ../docs)
        #[arg(long)]
        docs: Option<PathBuf>,
        /// Land it too: in devlab's own clones of the lab and the picture, each through `land`
        #[arg(long)]
        land: bool,
    },
    /// The phone's own health suite: put it on the phone, run it, read its latest report
    Suite {
        #[command(subcommand)]
        action: SuiteCmd,
    },
    /// devlab's door for agents: an MCP server on standard input and output
    Mcp,
    /// Watch the phone: health after every boot, silent when well, a board task when not
    Watch,
    /// The ways, held for agents: what Claude Code's hooks on the lab call (needs no manifest)
    Guard {
        #[command(subcommand)]
        event: GuardCmd,
    },
    /// A test kernel beside the stock one: tried once, falls back to stock by itself
    Kernel {
        #[command(subcommand)]
        action: KernelCmd,
    },
}

#[derive(Subcommand)]
enum GuardCmd {
    /// PreToolUse: the hook's JSON on standard input; a refused command exits 2, saying why
    Tool,
    /// SessionStart: the live practices and the unread messages
    Start,
}

#[derive(Subcommand)]
enum KernelCmd {
    /// Install it as a second boot entry with one try (MODULES.tgz holds lib/modules/RELEASE)
    Install {
        name: String,
        vmlinuz: PathBuf,
        dtb: PathBuf,
        modules: PathBuf,
        /// Extra kernel options for this entry only (e.g. dyndbg="file drivers/soundwire/* +p")
        #[arg(long, default_value = "")]
        cmdline: String,
    },
    /// Reboot into it once; any later boot is stock
    Try { name: String },
    /// What a timed-out try left on the boot partition
    Log,
    /// Remove it and its modules
    Remove { name: String },
}

#[derive(Subcommand)]
enum SuiteCmd {
    /// Put the suite on the phone (say `now`): the runner, the checks, the manifest's [health], the timer
    Put {
        now: String,
        /// The runner built for the phone (target/aarch64-unknown-linux-musl/release/devlab-health)
        #[arg(long)]
        runner: PathBuf,
    },
    /// Run it on the phone now, and say what it found; exit 1 when unhealthy
    Run,
    /// The latest report the phone keeps, as it is (JSON lines)
    Latest,
}

#[derive(Subcommand)]
enum PhoneCmd {
    /// Print the address that answers
    Find,
    /// Run the commands on standard input as the user
    Sh,
    /// Run the commands on standard input as root
    Root,
    /// Copy a local file to the phone
    Put { local: String, remote: String },
    /// Reboot, and wait until the phone answers again
    Reboot,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Cmd::Guard { event } = &cli.cmd {
        return match event {
            GuardCmd::Tool => guard::tool(),
            GuardCmd::Start => guard::start(),
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
            action: KernelCmd::Install { .. } | KernelCmd::Try { .. } | KernelCmd::Remove { .. },
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
                bail!("usage: devlab flash now IMAGE BOOTLOADER - it overwrites the phone");
            }
            flash::flash(&dev, &here, &image, &bootloader, dtb.as_deref(), as_is)?;
        }
        Cmd::Provision { now } => {
            if now != "now" {
                bail!("usage: devlab provision now - it shares this machine's connection with the phone for a while");
            }
            provision::provision(&dev, &here)?;
            let (well, said) = suite::after_change(&phone::Phone::find(&dev)?, &here, false)?;
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
                bail!("usage: devlab gather now [--docs DIR] [--land] - it writes into this lab and the picture");
            }
            println!("{}", gather::gather(&dev, &here, docs, land)?);
        }
        Cmd::Suite { action } => {
            let p = phone::Phone::find(&dev)?;
            match action {
                SuiteCmd::Put { now, runner } => {
                    if now != "now" {
                        bail!("usage: devlab suite put now --runner FILE - it changes the phone");
                    }
                    suite::put(&dev, &here, &p, &runner)?;
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
            }
        }
        Cmd::Mcp => mcp::serve(dev, here)?,
        Cmd::Watch => watch::watch(&dev, &here)?,
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
                KernelCmd::Try { name } => {
                    println!("{}", outcome::try_and_report(&dev, &p, &name)?);
                    // the boot the try came to - the test kernel or stock - is measured like any other
                    let now = phone::Phone::find(&dev)?;
                    let (well, said) = suite::after_change(&now, &here, true)?;
                    println!("{said}");
                    if !well {
                        std::process::exit(1);
                    }
                }
                KernelCmd::Log => kernel::log(&p)?,
                KernelCmd::Remove { name } => kernel::remove(&p, &name)?,
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
                    let (well, said) = suite::after_change(&back, &here, true)?;
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
