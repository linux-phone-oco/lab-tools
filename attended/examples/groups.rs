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
    // with slow-back: the way back takes a while, to be signalled again while it runs
    Step::new(Kind::Do, &format!("leave the {word} mark"), move || {
        let wait = if std::path::Path::new("slow-back").exists() {
            "touch backing; sleep 2; "
        } else {
            ""
        };
        phone().sh(&format!("{wait}echo {word} >> waybacks"))
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
                        // with retry-here: the step waits as wait_for_phone does, trying again every 5 seconds
                        if std::path::Path::new("retry-here").exists() {
                            return retry(60, 5, || phone().sh("touch holding; false"))
                                .map(|()| String::new());
                        }
                        // with hold-here: the step holds the run until a signal ends it
                        if std::path::Path::new("hold-here").exists() {
                            return phone().sh("touch holding; sleep 30");
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
    // with settle-here: a group that puts the phone back (settling), then a step that can still fail
    if std::path::Path::new("settle-here").exists() {
        run_group(
            group(
                "put the phone back",
                [Step::new(Kind::Do, "put it back", || {
                    phone().sh("test ! -e fail-settling")
                })],
            )
            .settling(),
        );
        step("work after settling", || phone().sh("test ! -e fail-after"));
    }
    // with settle-nested: the settling group sits one level deeper, inside a group that is still open
    if std::path::Path::new("settle-nested").exists() {
        run_group(
            group(
                "wrap up",
                [
                    Item::from(
                        group(
                            "put the phone back",
                            [Step::new(Kind::Do, "put it back", || phone().sh("true"))],
                        )
                        .settling(),
                    ),
                    Item::from(Step::new(Kind::Do, "work after settling", || {
                        phone().sh("test ! -e fail-after")
                    })),
                ],
            )
            .way_back(marks("wrapper")),
        );
    }
    step("all done", || phone().sh("true"));
}
