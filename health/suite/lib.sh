# The health suite's words, sourced by every check (POSIX sh - BusyBox's or not). A check prints one
# line per test:  pass|fail|skip TEST COMPONENT detail...  - lpoco-health makes the report of them.
# COMPONENT is a linux-phone-oco/docs component id. A skip says what could not be measured and why:
# what nobody measured is unknown, never broken. What a phone is expected to have comes from
# /etc/lpoco-health/health.env, written from the device's manifest (device.toml, [health]).
# A check that plays sound plays it quietly: at TEST_TONE_DBFS (the file's level) and TEST_VOLUME (the
# output volume, restored afterwards), both from the manifest's [hear] - quiet by default (-40 dBFS, 25%).
# A phone in a home must never shout.
[ ! -r /etc/lpoco-health/health.env ] || . /etc/lpoco-health/health.env  # a missing file must not end a POSIX shell

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

# A check that costs something to run - a call, an SMS, mobile data - runs at most once a day:
#   once_a_day sms-loopback || { say sms-loopback modem-sms skip "ran today"; exit 0; }
# succeeds the first time on a day and marks the day (before the costly part, so a check that fails
# half way does not spend again the same day); later that day it fails. Its place: the telephony layer.
once_a_day() {
	_stamp="/var/lib/lpoco-health/once/$1"
	_today=$(date +%Y-%m-%d)
	[ "$(cat "$_stamp" 2>/dev/null)" != "$_today" ] || return 1
	mkdir -p "${_stamp%/*}" && echo "$_today" > "$_stamp"
}

# json_field KEY LINE - KEY's value in one JSON line (a number, true/false or a plain string), as
# lpoco-health's own subcommands print it: lpoco-health tone find ... | json_field level_dbfs
json_field() { printf '%s' "$2" | sed -n "s/.*\"$1\": *\"\{0,1\}\([^,\"}]*\).*/\1/p"; }

# condition NAME - succeeds when the phone found out, at the start of this run, that NAME holds (no-sim,
# no-modem, no-network: conditions/*.sh print "NAME detail" lines, the runner passes them in LPOCO_CONDITIONS).
# What needs it says "skip: detail" - unknown, not broken - and nothing is written down: the next run finds out again.
condition() { printf '%s\n' "${LPOCO_CONDITIONS:-}" | grep -q "^$1 "; }
# skip_on NAME TEST COMPONENT - when NAME holds, says the test skipped for it and succeeds:  skip_on no-sim t c && exit 0
skip_on() {
	_d=$(printf '%s\n' "${LPOCO_CONDITIONS:-}" | sed -n "s/^$1 //p" | head -n1)
	[ -n "$_d" ] || return 1
	say "$2" "$3" skip "$_d"
}
# no_sim - the name the phone's own checks used before conditions: succeeds when the phone found out it has
# no SIM card (condition no-sim). Kept so that a check that calls it needs no change; new checks say skip_on.
no_sim() { condition no-sim; }
# switched_off TEST COMPONENT - a test somebody switched off (lpoco-health off TEST --reason ...) says so and
# succeeds: a check that costs something or makes noise asks it first.  switched_off mobile-data modem-data && exit 0
switched_off() {
	_l=$(awk -F'\t' -v t="$1" '$1 !~ /^#/ && $1 == t { print; exit }' /etc/lpoco-health/switches 2>/dev/null)
	[ -n "$_l" ] || return 1
	say "$1" "$2" skip "switched off: $(printf '%s' "$_l" | cut -f2) (by $(printf '%s' "$_l" | cut -f3), $(printf '%s' "$_l" | cut -f4))"
}
