// SPDX-License-Identifier: MIT OR Apache-2.0
//! osfact: the command. See cli.rs for the invocation surface.

mod cli;

use argenv_resolve::{InputResolveExt, OnProblems, Resolution};
use cli::Contract;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        print!(
            "{}",
            cli::usage("osfact", "os-factory: a device's installable image. Commands: install now --plan FILE --dry-run.")
        );
        return;
    }
    if Contract::VERSION.get_from(&r).unwrap_or(false) {
        println!("osfact {VERSION}");
        return;
    }
    if let Err(e) = run(&r) {
        eprintln!("osfact: {e}");
        std::process::exit(2);
    }
}

fn run(r: &Resolution) -> Result<(), String> {
    Contract::VERB
        .get_from(r)
        .ok_or("install needs the verb now: osfact install now --plan FILE --dry-run")?;
    let plan_file = Contract::PLAN
        .get_from(r)
        .ok_or("install needs --plan FILE")?;
    if !Contract::DRY_RUN.get_from(r).unwrap_or(false) {
        return Err(
            "install now runs only with --dry-run in this build: nothing is sent to a device yet"
                .into(),
        );
    }
    let text = std::fs::read_to_string(&plan_file).map_err(|e| format!("{plan_file}: {e}"))?;
    let plan = osfact::install::read(&text)?;
    for line in osfact::install::commands(&plan)? {
        println!("{line}");
    }
    Ok(())
}
