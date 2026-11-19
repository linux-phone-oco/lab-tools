# SPDX-License-Identifier: MIT OR Apache-2.0
# a suspended phone is quiet: the interrupts counted across one s2idle cycle (15 s, ended by an RTC wake
# alarm) stay under the manifest's threshold ([health] suspend_irq_max); over it, the check fails and names
# the three sources that counted most. Not measured - skipped, saying why - when there is no threshold (it
# comes from a reading of this phone, never a guess), no RTC wake alarm, no armed guard ([health]
# suspend_guard: a command that succeeds while the phone's watchdog is armed: a suspend that does not come
# back must reset the phone), or when the phone did not really suspend. It suspends only when all are had.
# Paths and seconds come from the environment for the test; the phone's own are the defaults.
. "${LPOCO_HEALTH_LIB:-/usr/lib/lpoco-health/lib.sh}"
T=suspend-irq-quiet C=deep-sleep
if absent deep-sleep; then say $T $C skip "the manifest says this phone has no deep sleep"; exit 0; fi
switched_off $T $C && exit 0
irq=${PROC_INTERRUPTS:-/proc/interrupts} rtc=${RTC_DIR:-/sys/class/rtc/rtc0} state=${POWER_STATE:-/sys/power/state}
stats=${SUSPEND_STATS:-/sys/power/suspend_stats/success} secs=${SUSPEND_SECONDS:-15}
if [ -z "${SUSPEND_IRQ_MAX:-}" ]; then
	say $T $C skip "not measured: no threshold in the manifest ([health] suspend_irq_max), and none is guessed"; exit 0
fi
if [ ! -w "$rtc/wakealarm" ] || [ ! -r "$rtc/since_epoch" ]; then
	say $T $C skip "not suspended: no RTC wake alarm to end the sleep ($rtc)"; exit 0
fi
if [ -z "${SUSPEND_GUARD:-}" ] || ! sh -c "$SUSPEND_GUARD" >/dev/null 2>&1; then
	if [ -n "${SUSPEND_GUARD:-}" ]; then why="suspend_guard \"$SUSPEND_GUARD\" fails"; else why="the manifest's [health] suspend_guard names none"; fi
	say $T $C skip "not suspended: the watchdog is not armed ($why)"; exit 0
fi
s0=$(cat "$stats" 2>/dev/null)
if [ -z "$s0" ]; then say $T $C skip "not suspended: the suspend counter ($stats) cannot be read, so a suspend could not be told from none"; exit 0; fi
now=$(cat "$rtc/since_epoch" 2>/dev/null)
case $now in ''|*[!0-9]*) say $T $C skip "not suspended: the RTC's clock cannot be read ($rtc/since_epoch)"; exit 0 ;; esac
tmp=$(mktemp -d) || exit 1
disarm() { { echo 0 > "$rtc/wakealarm"; } 2>/dev/null; }
trap 'disarm; rm -rf "$tmp"' EXIT
trap 'exit 1' HUP INT TERM
if ! cp "$irq" "$tmp/before" 2>/dev/null || ! grep -q ':' "$tmp/before"; then
	say $T $C skip "not suspended: $irq cannot be read"; exit 0
fi
target=$((now + secs))
{ echo 0 > "$rtc/wakealarm" && echo "$target" > "$rtc/wakealarm"; } 2>/dev/null
if [ "$(cat "$rtc/wakealarm" 2>/dev/null)" != "$target" ]; then
	say $T $C skip "not suspended: the RTC wake alarm did not take ($rtc/wakealarm)"; exit 0
fi
echo freeze > "$state"
sleep "${SUSPEND_SETTLE:-1}"
cp "$irq" "$tmp/after" 2>/dev/null && grep -q ':' "$tmp/after" || { say $T $C skip "not measured: $irq cannot be read after the cycle"; exit 0; }
t1=$(cat "$rtc/since_epoch" 2>/dev/null)
disarm
s1=$(cat "$stats" 2>/dev/null)
if [ "${s1:-0}" -le "$s0" ]; then
	say $T $C skip "not measured: the phone did not suspend (suspends this boot stayed at $s0)"; exit 0
fi
case $t1 in ''|*[!0-9]*) ;; *) if [ "$t1" -lt $((target - 1)) ]; then
	say $T $C skip "not measured: the phone woke after $((t1 - now)) s, not $secs (something else woke it)"; exit 0
fi ;; esac
# per interrupt: the count of all processors, then the name; the change over the cycle, most first
delta=$(awk '
	function load(f, into,   l, n, i, s, nm) {
		while ((getline l < f) > 0) {
			n = split(l, w, " "); if (w[1] !~ /:$/) continue
			s = 0; for (i = 2; i <= n && w[i] ~ /^[0-9]+$/; i++) s += w[i]
			nm = ""; for (; i <= n; i++) nm = nm (nm == "" ? "" : "_") w[i]
			into[w[1]] = s; names[w[1]] = (nm == "" ? w[1] : nm)
		}
		close(f)
	}
	BEGIN {
		load(ARGV[1], b); load(ARGV[2], a)
		for (k in a) { d = a[k] - (k in b ? b[k] : 0); if (d > 0) printf "%d %s %s\n", d, substr(k, 1, length(k) - 1), names[k] }
	}' "$tmp/before" "$tmp/after" | sort -rn)
total=$(printf '%s\n' "$delta" | awk '{ s += $1 } END { print s + 0 }')
top=$(printf '%s\n' "$delta" | head -n3 | awk 'NF { printf "%s%s (irq %s) %s", (n++ ? ", " : ""), $3, $2, $1 }')
detail="$total interrupts across one ${secs} s suspended cycle (at most $SUSPEND_IRQ_MAX); top: ${top:-none}"
if [ "$total" -le "$SUSPEND_IRQ_MAX" ]; then say $T $C pass "$detail"; else say $T $C fail "$detail"; fi
