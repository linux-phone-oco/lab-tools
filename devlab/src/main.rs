//! devlab: the device lab for Linux phones - for any device, from its manifest (device.toml).
//!
//!   devlab phone find              which address answers
//!   devlab phone sh   < script     run commands as the user (script on stdin)
//!   devlab phone root < script     run commands as root
//!   devlab phone put LOCAL REMOTE  copy a file to the phone
//!   devlab phone reboot            reboot and wait until it answers again
//!   devlab kernel install NAME VMLINUZ DTB MODULES.tgz | try NAME | log | remove NAME
//!                                  a test kernel beside stock, tried once, never stranding the phone

mod device;
mod kernel;
mod phone;

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
    /// A test kernel beside the stock one: tried once, falls back to stock by itself
    Kernel {
        #[command(subcommand)]
        action: KernelCmd,
    },
}

#[derive(Subcommand)]
enum KernelCmd {
    /// Install it as a second boot entry with one try (MODULES.tgz holds lib/modules/RELEASE)
    Install {
        name: String,
        vmlinuz: PathBuf,
        dtb: PathBuf,
        modules: PathBuf,
    },
    /// Reboot into it once; any later boot is stock
    Try { name: String },
    /// What a timed-out try left on the boot partition
    Log,
    /// Remove it and its modules
    Remove { name: String },
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
    let (dev, _) = device::Device::load(cli.device.as_deref(), &std::env::current_dir()?)?;
    match cli.cmd {
        Cmd::Kernel { action } => {
            let p = phone::Phone::find(&dev)?;
            match action {
                KernelCmd::Install {
                    name,
                    vmlinuz,
                    dtb,
                    modules,
                } => kernel::install(&p, &name, &vmlinuz, &dtb, &modules)?,
                KernelCmd::Try { name } => kernel::try_once(&p, &name)?,
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
                }
            }
        }
    }
    Ok(())
}
