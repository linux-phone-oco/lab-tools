# The health suite's words, sourced by every check (POSIX sh - BusyBox's or not). A check prints one
# line per test:  pass|fail|skip TEST COMPONENT detail...  - devlab-health makes the report of them.
# COMPONENT is a linux-phone-oco/docs component id. A skip says what could not be measured and why:
# what nobody measured is unknown, never broken. What a phone is expected to have comes from
# /etc/devlab/health.env, written from the device's manifest (device.toml, [health]).
# A check that plays sound plays it quietly: at TEST_TONE_DBFS (the file's level) and TEST_VOLUME (the
# output volume, restored afterwards), both from the manifest's [hear] - quiet by default (-40 dBFS, 25%).
# A phone in a home must never shout.
. /etc/devlab/health.env 2>/dev/null

# the kernel log of this boot, whole: dmesg is a ring that verbose drivers overwrite within minutes
klog() { journalctl -k -b 0 --no-pager -o short-monotonic 2>/dev/null || dmesg; }

say() { printf '%s %s %s %s\n' "$3" "$1" "$2" "$(printf '%s' "$4" | tr '\n' ' ')"; }
# check TEST COMPONENT CONDITION DETAIL - CONDITION "true" passes, anything else fails
check() { if [ "$3" = true ]; then say "$1" "$2" pass "$4"; else say "$1" "$2" fail "$4"; fi; }

# the user a graphical session runs as, if any
session_user() { stat -c %U /run/user/* 2>/dev/null | grep -v '^root$' | head -n1; }
as_user() {
	_u=$(session_user); [ -n "$_u" ] || return 1
	su "$_u" -c "XDG_RUNTIME_DIR=/run/user/$(id -u "$_u") $*" 2>/dev/null
}

in_list() { case " $2 " in *" $1 "*) return 0 ;; esac; return 1; }
# a part the manifest says this phone does not have: not measured, not failing
absent() { in_list "$1" "${ABSENT:-}"; }
