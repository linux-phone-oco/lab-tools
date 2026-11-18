# deep sleep: the SoC reached its lowest power state this boot, as the SoC itself counts it - on Qualcomm
# (qcom_stats, from the rpmh-stats or rpm-stats memory), CX collapse (cxsd) or the always-on subsystem's
# sleep (aosd). A phone that never suspended this boot (a wake lock held - on USB the lab keeps one) has
# not been measured: unknown, not broken. Suspended and the SoC still up is broken; the detail says which
# processors never slept and which wake locks were held, the usual reasons it stays up.
. "${LPOCO_HEALTH_LIB:-/usr/lib/lpoco-health/lib.sh}"
if absent deep-sleep; then say soc-sleeps deep-sleep skip "the manifest says this phone has no deep sleep"; exit 0; fi
# A controlled cycle of this boot (lpoco-health suspend cycle) is the measurement: the SoC's counters before and
# after one suspend, not a lifetime count. It says pass when the SoC slept in it, fail when the phone suspended
# and the SoC did not; a cycle of another boot, or none, is not read.
cyc=${LPOCO_HEALTH_CYCLE:-/var/lib/lpoco-health/suspend/cycle.json}
_b=$(cat "${BOOT_ID_FILE:-/proc/sys/kernel/random/boot_id}" 2>/dev/null)
if [ -r "$cyc" ] && [ -n "$_b" ] && [ "$(json_field boot "$(cat "$cyc")")" = "$_b" ]; then
	_c=$(cat "$cyc"); _d="one controlled cycle of $(json_field seconds "$_c") s, asleep $(json_field asleep_s "$_c") s"
	case $_c in
	*'"soc_slept":true'*) say soc-sleeps deep-sleep pass "$_d: the SoC reached its lowest state"; exit 0 ;;
	*'"suspended":true'*) say soc-sleeps deep-sleep fail "$_d: suspended, yet the SoC never reached its lowest state"; exit 0 ;;
	esac
fi
st=/sys/kernel/debug/qcom_stats
count() { awk '/^Count:/ { print $2; exit }' "$st/$1" 2>/dev/null; }
slept=$(cat /sys/power/suspend_stats/success 2>/dev/null)
if [ ! -r "$st/cxsd" ] && [ ! -r "$st/aosd" ]; then
	say soc-sleeps deep-sleep skip "the SoC's own sleep counters cannot be read (no qcom_stats cxsd or aosd); suspends this boot: ${slept:-?}"
	exit 0
fi
cx=$(count cxsd); ao=$(count aosd)
awake=""
for p in adsp cdsp modem slpi; do [ "$(count "$p")" = 0 ] && awake="$awake $p"; done
detail="cxsd ${cx:-?}, aosd ${ao:-?}; suspends this boot: ${slept:-?}; processors never asleep:${awake:- none}; wake locks: $(cat /sys/power/wake_lock 2>/dev/null)"
if [ "${cx:-0}" -gt 0 ] || [ "${ao:-0}" -gt 0 ]; then
	say soc-sleeps deep-sleep pass "$detail"
elif [ "${slept:-0}" -gt 0 ]; then
	say soc-sleeps deep-sleep fail "suspended, yet the SoC never reached its lowest state: $detail"
else
	say soc-sleeps deep-sleep skip "not measured: the phone has not suspended this boot; $detail"
fi
