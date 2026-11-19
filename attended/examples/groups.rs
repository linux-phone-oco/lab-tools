#!/usr/bin/env rust-script
//! An attended procedure that chains named groups: each group is one line in the entry file, its steps are logged
//! under its name, --dry-run lists them nested, and a group may carry a way back that runs when a step after its
//! start fails. A group inside a group is a step list like any other.
//!
//! ```cargo
//! [dependencies]
//! lpoco-attended = { git = "https://github.com/linux-phone-oco/lab-tools", tag = "lpoco-attended-v0.37.1" }
//! ```
// SPDX-License-Identifier: MIT OR Apache-2.0

use lpoco_attended::prelude::*;

fn marks(word: &'static str) -> Step {
    Step::new(Kind::Do, &format!("leave the {word} mark"), move || {
        phone().sh(&format!("echo {word} >> waybacks"))
    })
}

fn main() {
    let _run = procedure("groups", "xiaomi-surya");

    run_group(
        group(
            "reach the phone",
            [
                Step::new(Kind::Check, "the phone answers", || phone().sh("true")),
                Step::new(Kind::Check, "the shell is up", || phone().sh("echo up")),
            ],
        )
        .way_back(marks("outer")),
    );
    run_group(group(
        "do the work",
        [
            Item::from(
                group(
                    "the inner part",
                    [Step::new(Kind::Do, "work unless told not to", || {
                        if std::path::Path::new("panic-here").exists() {
                            panic!("boom");
                        }
                        phone().sh("test ! -e fail-here")
                    })],
                )
                .way_back(marks("inner")),
            ),
            Item::from(Step::new(Kind::Check, "the work is done", || {
                phone().sh("true")
            })),
        ],
    ));
    step("all done", || phone().sh("true"));
}
