// SPDX-License-Identifier: MIT OR Apache-2.0
//! lpoco-manifest's invocation surface, declared once with argenv:
//!
//!   lpoco-manifest schema     print the JSON Schema of device.toml, projected from the types
//!   lpoco-manifest --help     this usage

use argenv_resolve::*;

contract! {
    COMMAND: String = Input {
        key: "command",
        ty: Type::Enum,
        allowed: &["schema"],
        arg: Some(Arg::positional(0)),
        summary: "schema: print the JSON Schema of device.toml.",
        ..Input::EMPTY
    };

    HELP: bool = Input {
        key: "help",
        ty: Type::Bool,
        default: Some(false),
        arg: Some(Arg { short: Some('h'), ..Arg::long("help") }),
        summary: "Print this usage and stop.",
        ..Input::EMPTY
    };
}

fn main() {
    let r = Contract::parse_and_lint(OnProblems::FailOnError);
    if Contract::HELP.get_from(&r).unwrap_or(false) {
        println!("Usage: lpoco-manifest schema\n\nThe JSON Schema of device.toml, projected from the types every lpoco program reads.");
        return;
    }
    match Contract::COMMAND.get_from(&r).as_deref() {
        Some("schema") => print!("{}", lpoco_manifest::schema()),
        _ => {
            eprintln!("usage: lpoco-manifest schema");
            std::process::exit(2);
        }
    }
}
