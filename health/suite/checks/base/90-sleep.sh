# deep sleep: the SoC reached its lowest power state this boot, as the SoC itself counts it - on Qualcomm
# (qcom_stats, from the rpmh-stats or rpm-stats memory), CX collapse (cxsd) or the always-on subsystem's
# sleep (aosd). A phone that never suspended this boot (a wake lock held - on USB the lab keeps one) has
# not been measured: unknown, not broken. Suspended and the SoC still up is broken; the detail says which
# processors never slept and which wake locks were held, the usual reasons it stays up.
. /usr/lib/devlab/health/lib.sh
if absent deep-sleep; then say soc-sleeps deep-sleep skip "the manifest says this phone has no deep sleep"; exit 0; fi
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
