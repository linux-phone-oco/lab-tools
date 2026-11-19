// SPDX-License-Identifier: MIT OR Apache-2.0
//! The bench: who does a physical act. A step names the act (press the power button, unplug the cable, look for a
//! logo); a [`Bench`] does it. The person is the bench that is always there; a lane that has a relay, a servo, a
//! camera or a microphone puts it on the bench with [`bench`] and it does the acts it claims, a person the rest.

use std::sync::{Arc, Mutex};

/// The doer's name for the person at the bench.
pub const PERSON: &str = "person";

/// What is done or observed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ActKind {
    PressPower,
    HoldButton(String),
    UnplugUsb,
    ReplugUsb,
    Touch(String),
    Swipe(String),
    /// something to see on the phone
    LookFor(String),
    /// something to hear from the phone
    ListenFor(String),
    /// the author's own words: "what to do / what to check"
    Confirm(String),
}

/// One act and what to check once it is done.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Act {
    pub kind: ActKind,
    pub check: String,
}

impl Act {
    pub fn new(kind: ActKind, check: &str) -> Act {
        let check = if check.trim().is_empty() {
            "it went as expected"
        } else {
            check
        };
        Act {
            kind,
            check: check.to_string(),
        }
    }

    /// What is done, as a sentence to a person ("press the power button").
    pub fn to_do(&self) -> String {
        match &self.kind {
            ActKind::PressPower => "press the power button".into(),
            ActKind::HoldButton(b) => format!("hold the {b} button"),
            ActKind::UnplugUsb => "unplug the USB cable".into(),
            ActKind::ReplugUsb => "plug the USB cable back in".into(),
            ActKind::Touch(t) => format!("touch {t}"),
            ActKind::Swipe(s) => format!("swipe {s}"),
            ActKind::LookFor(_) => "look at the phone".into(),
            ActKind::ListenFor(_) => "listen to the phone".into(),
            ActKind::Confirm(w) => w.split_once(" / ").map_or(w.as_str(), |s| s.0).into(),
        }
    }

    /// The step's name in the run and the log.
    pub fn name(&self) -> String {
        match &self.kind {
            ActKind::LookFor(w) => format!("look for {w}"),
            ActKind::ListenFor(w) => format!("listen for {w}"),
            ActKind::Confirm(_) => format!("ask the author: {}", self.to_do()),
            _ => self.to_do(),
        }
    }

    /// What the person is asked to check afterwards.
    pub fn to_check(&self) -> String {
        match &self.kind {
            ActKind::LookFor(w) => format!("you can see {w}"),
            ActKind::ListenFor(w) => format!("you can hear {w}"),
            ActKind::Confirm(w) => w
                .split_once(" / ")
                .map_or("it went as expected", |s| s.1)
                .into(),
            _ => self.check.clone(),
        }
    }
}

/// Something that can do physical acts. It claims the ones it can and does them; an act no bench claims goes to the
/// person. A failure is the act not done (or not as expected): the run stops there and names it. `do_act` does the act
/// and checks `act.check` itself (a camera looks, a microphone listens): `Ok` is trusted.
pub trait Bench: Send + Sync {
    /// The doer's name, as a dry run lists it.
    fn name(&self) -> &str;
    fn claims(&self, act: &Act) -> bool;
    fn do_act(&self, act: &Act) -> Result<(), String>;
}

static BENCHES: Mutex<Vec<Arc<dyn Bench>>> = Mutex::new(Vec::new());

/// Put a bench on the bench, after [`procedure`](crate::procedure) and before the steps it should do. The first that
/// claims an act does it. A step takes its doer when it is built: a bench put on the bench after a step was built does not do it.
pub fn bench(b: impl Bench + 'static) {
    BENCHES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .push(Arc::new(b));
}

/// The bench that does the act, none for the person.
pub(crate) fn doer(act: &Act) -> Option<Arc<dyn Bench>> {
    let all = BENCHES.lock().unwrap_or_else(|e| e.into_inner()).clone();
    all.into_iter().find(|b| b.claims(act))
}
