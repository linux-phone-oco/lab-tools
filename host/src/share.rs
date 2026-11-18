//! What root is for, from lpoco's roof (decision of the author, 2026-11-18: no standing root). Every root need of
//! the lpoco tools is declared here once, as a `RootNeed`; plan, prepare and check read the same list, so what
//! `lpoco host plan` prints and what the lab holds cannot drift apart.
//!
//! There is no passwordless root: `lpoco host prepare now` runs each root command behind `sudo` with the author's
//! typed password (the default prefix; `LPOCO_ROOT` names another), and only to install the files below.
//! At run time root is two narrow, fixed things, both installed here:
//!   - `lpoco-share@USB--UPLINK.service`, a system unit that shares the lab's connection with a phone on USB
//!     while it runs, and undoes it when it stops (forwarding, one nftables table, a ufw route);
//!   - a polkit rule that lets the lab user start and stop units of exactly that name and nothing else.

pub const SHARE_UNIT_DST: &str = "/etc/systemd/system/lpoco-share@.service";
pub const SHARE_HELPER_DST: &str = "/usr/local/libexec/lpoco-share";
pub const POLKIT_DST: &str = "/etc/polkit-1/rules.d/50-lpoco.rules";

/// A thing the lpoco tools need root for.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RootNeed {
    /// /etc/udev/rules.d/70-lpoco-CODENAME.rules, udevadm reload and trigger: a device's nodes open to the lab user.
    UdevRules,
    /// /etc/modules-load.d/lpoco.conf and modprobe: the gadget functions' modules.
    ModulesLoad,
    /// A NetworkManager or systemd-networkd file for a device's USB network, and its reload.
    LinkConfig,
    /// The share unit, its helper and the polkit rule: the phone's connection to the internet while it is provisioned.
    PhoneShare,
}

impl RootNeed {
    pub const ALL: &'static [RootNeed] = &[
        RootNeed::UdevRules,
        RootNeed::ModulesLoad,
        RootNeed::LinkConfig,
        RootNeed::PhoneShare,
    ];

    pub fn name(self) -> &'static str {
        match self {
            RootNeed::UdevRules => "udev rules",
            RootNeed::ModulesLoad => "modules to load",
            RootNeed::LinkConfig => "a USB network link",
            RootNeed::PhoneShare => "the phone's shared connection",
        }
    }

    /// When root is asked, and who is at the keyboard.
    pub fn when(self) -> &'static str {
        match self {
            RootNeed::PhoneShare => {
                "installed by prepare; at run time the lab user starts and stops the unit through polkit, no password, no sudo"
            }
            _ => "only in `prepare now`, behind sudo with the author's password",
        }
    }
}

/// The helper the unit runs, as root. Fixed text: the instance name is its only input, and it is checked here.
pub fn helper_text() -> String {
    r#"#!/bin/sh
# lpoco-share - written by `lpoco host prepare now`; edit lpoco-host, not this file.
# Shares this machine's connection with a phone on USB while lpoco-share@USB--UPLINK.service runs.
# USB and UPLINK are interface names; the phone's network is the USB interface's own /24.
set -eu
action=${1:?start or stop}; inst=${2:?USB--UPLINK}
usb=${inst%%--*}; up=${inst#*--}
for n in "$usb" "$up"; do
	case "$n" in ''|*[!A-Za-z0-9._-]*) echo "lpoco-share: not an interface name: $n" >&2; exit 2 ;; esac
	# a stop must work after the phone is unplugged: only a start needs the interface to be there
	if [ "$action" = start ] && [ ! -d "/sys/class/net/$n" ]; then echo "lpoco-share: no interface $n" >&2; exit 2; fi
done
state=/run/lpoco-share; table=lpoco-share
case "$action" in
start)
	cidr=$(ip -4 -o addr show dev "$usb" | awk '{print $4; exit}')
	case "$cidr" in */24) ;; *) echo "lpoco-share: $usb has no /24 address ($cidr)" >&2; exit 2 ;; esac
	net=${cidr%.*}.0/24
	mkdir -p "$state"
	[ -f "$state/$inst.fwd" ] || cat /proc/sys/net/ipv4/ip_forward > "$state/$inst.fwd"
	sysctl -qw net.ipv4.ip_forward=1
	if command -v ufw >/dev/null 2>&1 && ufw status 2>/dev/null | grep -q 'Status: active'; then
		touch "$state/$inst.ufw"
		ufw route allow in on "$usb" out on "$up" >/dev/null
	fi
	nft add table ip "$table"
	nft add chain ip "$table" out '{ type nat hook postrouting priority 100; }'
	nft add rule ip "$table" out ip saddr "$net" oifname "$up" masquerade
	;;
stop)
	nft delete table ip "$table" 2>/dev/null || true
	if [ -f "$state/$inst.ufw" ]; then
		ufw route delete allow in on "$usb" out on "$up" >/dev/null 2>&1 || true
		rm -f "$state/$inst.ufw"
	fi
	if [ -f "$state/$inst.fwd" ]; then
		[ "$(cat "$state/$inst.fwd")" = 1 ] || sysctl -qw net.ipv4.ip_forward=0
		rm -f "$state/$inst.fwd"
	fi
	;;
*) echo "lpoco-share: start or stop" >&2; exit 2 ;;
esac
"#
    .to_string()
}

pub fn unit_text() -> String {
    format!(
        "[Unit]\nDescription=lpoco share: the lab's connection for a phone on USB (%i)\n\n[Service]\nType=oneshot\nRemainAfterExit=yes\nExecStart={SHARE_HELPER_DST} start %i\nExecStop={SHARE_HELPER_DST} stop %i\n"
    )
}

/// The polkit rule: `user` may start and stop `lpoco-share@*.service` and no other unit, verb or user.
pub fn polkit_text(user: &str) -> String {
    format!(
        "// lpoco host: written by `lpoco host prepare now`; edit lpoco-host, not this file.\npolkit.addRule(function(action, subject) {{\n    if (action.id == \"org.freedesktop.systemd1.manage-units\" &&\n        subject.user == \"{user}\" &&\n        (action.lookup(\"verb\") == \"start\" || action.lookup(\"verb\") == \"stop\") &&\n        /^lpoco-share@[A-Za-z0-9._-]+--[A-Za-z0-9._-]+\\.service$/.test(action.lookup(\"unit\"))) {{\n        return polkit.Result.YES;\n    }}\n}});\n"
    )
}

/// A user name safe to put in the rule: letters, digits, `_`, `-`, `.`.
pub fn valid_user(user: &str) -> bool {
    !user.is_empty()
        && user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_names_one_user_and_one_unit_pattern() {
        let r = polkit_text("mll");
        assert!(r.contains("subject.user == \"mll\""));
        assert!(r.contains("lpoco-share@"));
        assert!(r.contains("\"start\"") && r.contains("\"stop\""));
        assert!(!r.contains("restart") && !r.contains("enable"));
    }

    #[test]
    fn a_user_name_cannot_close_the_rule() {
        assert!(valid_user("mll") && valid_user("a.b-c_d"));
        for bad in ["", "a\"b", "a b", "a);b", "x/y"] {
            assert!(!valid_user(bad), "{bad:?}");
        }
    }

    #[test]
    fn the_unit_runs_the_helper_for_its_instance_only() {
        let u = unit_text();
        assert!(u.contains(&format!("ExecStart={SHARE_HELPER_DST} start %i")));
        assert!(u.contains(&format!("ExecStop={SHARE_HELPER_DST} stop %i")));
    }

    #[test]
    fn the_helper_refuses_names_that_are_not_interfaces() {
        let h = helper_text();
        assert!(h.contains("*[!A-Za-z0-9._-]*"));
        assert!(h.contains("/sys/class/net/$n"));
    }

    /// The helper's refusals run for real, as the lab user: they exit before any change.
    #[test]
    fn the_helper_runs_its_refusals() {
        let dir = std::env::temp_dir().join(format!("lpoco-share-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("h.sh");
        std::fs::write(&p, helper_text()).unwrap();
        let run = |a: &[&str]| {
            std::process::Command::new("sh")
                .arg(&p)
                .args(a)
                .output()
                .unwrap()
        };
        assert_eq!(run(&["start", "no such;x--lo"]).status.code(), Some(2));
        assert_eq!(run(&["start", "nonexistent0--lo"]).status.code(), Some(2));
        assert_eq!(run(&["bogus", "lo--lo"]).status.code(), Some(2));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
