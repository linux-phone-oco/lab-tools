//! devlab: the device lab for Linux phones - for any device, from its manifest (device.toml).
//!
//!   devlab phone find              which address answers
//!   devlab phone sh   < script     run commands as the user (script on stdin)
//!   devlab phone root < script     run commands as root
//!   devlab phone put LOCAL REMOTE  copy a file to the phone
//!   devlab phone reboot            reboot and wait until it answers again

mod device;
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
