// SPDX-License-Identifier: MIT OR Apache-2.0
//! What the lab machine's own kernel log says about the phone's USB device while a try waits for it
//! (backlog kernel-try-usb-timeline): a phone that froze after its reboot stays connected on the host
//! with no new enumeration, a phone that is coming up enumerates twice (initramfs, then systemd), a phone
//! that never reached the bus leaves nothing. Read-only: the log is `journalctl -k --utc -o short-iso`,
//! the phone is found by its declared `usb_id` and `serial`, never by a bus number.

use std::collections::HashMap;

/// A device that has sat connected this long, with nothing after it, is not coming up any more.
pub const SETTLE_SECONDS: u64 = 60;

/// How far before the try the host's log is read, so a phone that never left the bus is still seen.
pub const LOOKBACK_SECONDS: u64 = 3600;

/// How the phone is recognised on the host's bus: `[[host.gadget]]` usb_id and serial.
pub struct Wanted<'a> {
    pub usb_id: &'a str,
    pub serial: &'a str,
}

#[derive(Debug, PartialEq)]
pub enum Timeline {
    /// Connected and still connected, nothing re-enumerated since: the phone froze with its gadget up.
    Frozen { device: u32, since: u64 },
    /// Enumerations close together, the last one recent: a boot is in progress.
    Reenumerating { enumerations: usize, last: u64 },
    /// No event for the phone, or it left and has not come back.
    Absent { gone_at: Option<u64> },
}

#[derive(Debug, PartialEq)]
enum Event {
    Connect { port: String, device: u32, at: u64 },
    Disconnect { at: u64 },
}

/// The days from 1970-01-01 to a civil date (proleptic Gregorian).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Seconds since the epoch of a `short-iso` stamp (`2026-11-19T14:25:41+00:00`, read as UTC: the log is asked with --utc).
fn stamp(line: &str) -> Option<(u64, &str)> {
    let (when, rest) = line.split_once(' ')?;
    let n = |a: usize, b: usize| when.get(a..b)?.parse::<i64>().ok();
    let days = days_from_civil(n(0, 4)?, n(5, 7)?, n(8, 10)?);
    let secs = days * 86400 + n(11, 13)? * 3600 + n(14, 16)? * 60 + n(17, 19)?;
    u64::try_from(secs).ok().map(|s| (s, rest))
}

/// The port and what follows it in a kernel `usb P: ...` message.
fn usb_message(rest: &str) -> Option<(&str, &str)> {
    let msg = rest.split_once(" kernel: ")?.1.strip_prefix("usb ")?;
    msg.split_once(": ")
}

fn field<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    let after = text.split_once(key)?.1;
    after.split(|c: char| c == ',' || c.is_whitespace()).next()
}

/// What the journal says happened to the wanted device up to `until` (seconds since the epoch).
pub fn read(journal: &str, wanted: &Wanted, until: u64) -> Timeline {
    let mut events: Vec<Event> = Vec::new();
    // per port: the connection the kernel announced last, and whether the phone's id was confirmed for it
    let mut announced: HashMap<&str, (u32, u64)> = HashMap::new();
    let mut ours: HashMap<&str, u32> = HashMap::new();
    for line in journal.lines() {
        let Some((at, rest)) = stamp(line) else {
            continue;
        };
        if at > until {
            continue;
        }
        let Some((port, msg)) = usb_message(rest) else {
            continue;
        };
        if let Some(n) = msg
            .strip_prefix("new ")
            .and_then(|m| m.split_once(" USB device number "))
        {
            if let Some(device) = n.1.split_whitespace().next().and_then(|d| d.parse().ok()) {
                announced.insert(port, (device, at));
                ours.remove(port);
            }
        } else if msg.starts_with("New USB device found") {
            let id = format!(
                "{}:{}",
                field(msg, "idVendor=").unwrap_or(""),
                field(msg, "idProduct=").unwrap_or("")
            );
            if let (true, Some(&(device, seen))) = (id == wanted.usb_id, announced.get(port)) {
                ours.insert(port, device);
                events.push(Event::Connect {
                    port: port.to_string(),
                    device,
                    at: seen,
                });
            }
        } else if let Some(serial) = msg.strip_prefix("SerialNumber: ") {
            if let Some(device) = ours.get(port).copied() {
                if serial.trim() != wanted.serial {
                    ours.remove(port);
                    events
                        .retain(|e| !matches!(e, Event::Connect { port: p, device: d, .. } if p == port && *d == device));
                }
            }
        } else if let Some(n) = msg.strip_prefix("USB disconnect, device number ") {
            if let (Some(device), Ok(n)) = (ours.get(port).copied(), n.trim().parse::<u32>()) {
                if device == n {
                    ours.remove(port);
                    events.push(Event::Disconnect { at });
                }
            }
        }
    }
    verdict(&events, until)
}

fn verdict(events: &[Event], until: u64) -> Timeline {
    match events.last() {
        None => Timeline::Absent { gone_at: None },
        Some(Event::Disconnect { at }) => Timeline::Absent { gone_at: Some(*at) },
        Some(Event::Connect { device, at, .. }) if until.saturating_sub(*at) > SETTLE_SECONDS => {
            Timeline::Frozen {
                device: *device,
                since: *at,
            }
        }
        Some(Event::Connect { at, .. }) => {
            let mut enumerations = 0;
            let mut next = *at;
            for e in events.iter().rev() {
                if let Event::Connect { at, .. } = e {
                    if next.saturating_sub(*at) > SETTLE_SECONDS {
                        break;
                    }
                    enumerations += 1;
                    next = *at;
                }
            }
            Timeline::Reenumerating {
                enumerations,
                last: *at,
            }
        }
    }
}

fn clock(t: u64) -> String {
    let s = t % 86400;
    format!("{:02}:{:02}:{:02} UTC", s / 3600, s % 3600 / 60, s % 60)
}

/// In plain words.
pub fn say(t: &Timeline) -> String {
    match t {
        Timeline::Frozen { device, since } => format!(
            "frozen: device {device} connected since {}, no re-enumeration",
            clock(*since)
        ),
        Timeline::Reenumerating { enumerations, last } => format!(
            "re-enumerating: {enumerations} enumeration(s), the last at {}",
            clock(*last)
        ),
        Timeline::Absent { gone_at: None } => {
            "absent: the host saw no event for the phone's USB id".into()
        }
        Timeline::Absent { gone_at: Some(t) } => {
            format!(
                "absent: the phone disconnected at {} and has not come back",
                clock(*t)
            )
        }
    }
}

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds since the epoch, now.
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The verdict for gadget `g` from a journal: a journal with no kernel line at all is not known, never an absent phone.
fn describe(g: &lpoco_manifest::Gadget, journal: &str, until: u64) -> String {
    let usb_id = g.usb_id.as_deref().unwrap_or_default();
    if !journal.contains(" kernel: ") {
        return format!("host USB timeline ({usb_id}): not known yet - the host's kernel log holds no line in the window");
    }
    let wanted = Wanted {
        usb_id,
        serial: &g.serial,
    };
    format!(
        "host USB timeline ({usb_id}): {}",
        say(&read(journal, &wanted, until))
    )
}

/// What the host saw of the phone since `since`, in words: its kernel log read for the first declared
/// gadget with a usb_id (times passed as arguments, never in a shell string). Read-only; a log that
/// cannot be read is said so, never taken for an absent phone.
pub fn report(gadgets: &[lpoco_manifest::Gadget], since: u64) -> String {
    let Some(g) = gadgets.iter().find(|g| g.usb_id.is_some()) else {
        return "host USB timeline: not known yet - device.toml declares no [[host.gadget]] usb_id"
            .into();
    };
    let until = now();
    // from an hour before the try: a phone that froze before it left the bus has its connect line there
    let since = since.saturating_sub(LOOKBACK_SECONDS);
    let out = Command::new("journalctl")
        .args(["-k", "--utc", "-o", "short-iso", "--no-pager"])
        .arg(format!("--since=@{since}"))
        .arg(format!("--until=@{until}"))
        .output();
    match out {
        Ok(o) if o.status.success() => describe(g, &String::from_utf8_lossy(&o.stdout), until),
        Ok(o) => format!(
            "host USB timeline: not known yet - journalctl said {}",
            String::from_utf8_lossy(&o.stderr)
                .lines()
                .next()
                .unwrap_or("nothing")
        ),
        Err(e) => format!("host USB timeline: not known yet - journalctl did not run: {e}"),
    }
}

#[cfg(test)]
fn epoch(iso: &str) -> Option<u64> {
    stamp(&format!("{iso} x")).map(|s| s.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const WANTED: Wanted = Wanted {
        usb_id: "18d1:d001",
        serial: "surya-lab",
    };

    fn enumerated(at: &str, device: u32) -> String {
        format!(
            "{at}+00:00 lab kernel: usb 3-1: new high-speed USB device number {device} using xhci_hcd\n\
             {at}+00:00 lab kernel: usb 3-1: New USB device found, idVendor=18d1, idProduct=d001, bcdDevice= 7.01\n\
             {at}+00:00 lab kernel: usb 3-1: SerialNumber: surya-lab\n             {at}+00:00 lab kernel: usb 3-1: Product: surya\n"
        )
    }
    fn left(at: &str, device: u32) -> String {
        format!("{at}+00:00 lab kernel: usb 3-1: USB disconnect, device number {device}\n")
    }

    /// The host's journal of 2026-11-19 14:25-14:48 (surya-lab research
    /// 2026-11-19-calls4-hang-was-the-reboot-out-of-calls-plus): d001 at 14:25:41, again as device 38 at
    /// 14:25:54, then nothing until the author's power cycle at 14:47:45 and the next boot at 14:48:04.
    fn the_calls4_night() -> String {
        let mut j = String::from("2026-11-19T14:20:00+00:00 lab kernel: usb 1-2: new full-speed USB device number 5 using xhci_hcd\n");
        j += &enumerated("2026-11-19T14:25:41", 37);
        j += &left("2026-11-19T14:25:53", 37);
        j += &enumerated("2026-11-19T14:25:54", 38);
        j += &left("2026-11-19T14:47:45", 38);
        j += &enumerated("2026-11-19T14:48:04", 40);
        j += &left("2026-11-19T14:48:15", 40);
        j += &enumerated("2026-11-19T14:48:16", 41);
        j
    }

    fn at(iso: &str) -> u64 {
        epoch(iso).unwrap()
    }

    #[test]
    fn kernel_try_reports_usb_timeline() {
        let j = the_calls4_night();
        let frozen = read(&j, &WANTED, at("2026-11-19T14:47:00"));
        assert_eq!(
            frozen,
            Timeline::Frozen {
                device: 38,
                since: at("2026-11-19T14:25:54")
            }
        );
        assert_eq!(
            say(&frozen),
            "frozen: device 38 connected since 14:25:54 UTC, no re-enumeration"
        );
        // a normal boot: two enumerations 12 s apart, the window ending right after
        let booting = read(&j, &WANTED, at("2026-11-19T14:48:30"));
        assert_eq!(
            booting,
            Timeline::Reenumerating {
                enumerations: 2,
                last: at("2026-11-19T14:48:16")
            }
        );
        // no events: another phone's id, another serial, or nothing at all
        let other = Wanted {
            usb_id: "18d1:4ee0",
            serial: "surya-lab",
        };
        assert_eq!(
            read(&j, &other, at("2026-11-19T14:48:30")),
            Timeline::Absent { gone_at: None }
        );
        let other = Wanted {
            usb_id: "18d1:d001",
            serial: "walleye",
        };
        assert_eq!(
            read(&j, &other, at("2026-11-19T14:48:30")),
            Timeline::Absent { gone_at: None }
        );
        assert_eq!(
            read("", &WANTED, at("2026-11-19T14:48:30")),
            Timeline::Absent { gone_at: None }
        );
        // it left and is not back
        let mid = read(&j, &WANTED, at("2026-11-19T14:47:50"));
        assert_eq!(
            mid,
            Timeline::Absent {
                gone_at: Some(at("2026-11-19T14:47:45"))
            }
        );
        assert!(say(&mid).starts_with("absent: the phone disconnected at 14:47:45 UTC"));
    }

    #[test]
    fn a_second_port_and_another_serial_never_take_the_phones_events() {
        let mut j = enumerated("2026-11-19T14:25:41", 37);
        // another device of the same id on bus 1, the phone's own device number, another serial
        j += "2026-11-19T14:25:50+00:00 lab kernel: usb 1-4: new high-speed USB device number 37 using xhci_hcd\n\
               2026-11-19T14:25:50+00:00 lab kernel: usb 1-4: New USB device found, idVendor=18d1, idProduct=d001, bcdDevice= 7.01\n\
               2026-11-19T14:25:50+00:00 lab kernel: usb 1-4: SerialNumber: walleye\n\
               2026-11-19T14:25:51+00:00 lab kernel: usb 1-4: USB disconnect, device number 37\n";
        assert_eq!(
            read(&j, &WANTED, at("2026-11-19T14:30:00")),
            Timeline::Frozen {
                device: 37,
                since: at("2026-11-19T14:25:41")
            }
        );
    }

    #[test]
    fn the_settle_time_is_sixty_seconds_and_one_enumeration_reads_as_re_enumerating() {
        let j = enumerated("2026-11-19T14:25:41", 37);
        let t = at("2026-11-19T14:25:41");
        assert_eq!(
            read(&j, &WANTED, t + 60),
            Timeline::Reenumerating {
                enumerations: 1,
                last: t
            }
        );
        assert_eq!(
            read(&j, &WANTED, t + 61),
            Timeline::Frozen {
                device: 37,
                since: t
            }
        );
    }

    #[test]
    fn a_log_without_kernel_lines_is_not_known_and_a_gadget_without_usb_id_is_said() {
        let g = lpoco_manifest::Gadget {
            serial: "s".into(),
            access: vec![],
            net_name: None,
            net_address: None,
            usb_id: Some("18d1:d001".into()),
        };
        assert!(describe(&g, "-- No entries --\n", 0).contains("not known yet"));
        assert!(report(&[], 0).contains("declares no [[host.gadget]] usb_id"));
    }

    #[test]
    fn the_stamp_is_utc_seconds() {
        assert_eq!(epoch("2026-11-19T14:25:41"), Some(1_795_098_341));
    }
}
