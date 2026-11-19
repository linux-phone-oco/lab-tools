// SPDX-License-Identifier: MIT OR Apache-2.0
//! The file `lpoco attended new` writes: attended/examples/attended.rs for this device, its dependency on this
//! release (or a checkout) and on the repo's own steps crate when procedures/steps is one.

const REPO: &str = "https://github.com/linux-phone-oco/lab-tools";

pub fn procedure(name: &str, codename: &str, lib: Option<&str>, steps: Option<String>) -> String {
    let dep = match lib {
        Some(path) => format!(
            "lpoco-attended = {{ path = {:?} }}",
            std::path::absolute(path)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| path.to_string())
        ),
        None => format!(
            "lpoco-attended = {{ git = \"{REPO}\", tag = \"lpoco-attended-v{}\" }}",
            env!("CARGO_PKG_VERSION")
        ),
    };
    let steps_dep = steps
        .map(|n| format!("//! {n} = {{ path = \"steps\" }}\n"))
        .unwrap_or_default();
    format!(
        r#"#!/usr/bin/env rust-script
//! {name}: an attended procedure for {codename}. A plain list of named steps; a failing step stops the run and is named.
//! lpoco attended run {name} --dry-run   walks every step and touches nothing; --log FILE is the log to file.
//!
//! ```cargo
//! [dependencies]
//! {dep}
{steps_dep}//! ```

use lpoco_attended::prelude::*;

fn main() {{
    let _run = procedure("{name}", "{codename}");

    step("the phone answers", || retry(3, 1, || phone().sh("true")));
    ask_author("what the author does / what the author sees");
    check("the kernel is the one we expect", || {{
        phone().sh("uname -r").map(|k| !k.is_empty())
    }});
}}
"#
    )
}
