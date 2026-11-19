# SPDX-License-Identifier: MIT OR Apache-2.0
# kernel and system: no oops or panic, no new warning, no processor crashing, no unit failed - each
# known exception named in the manifest with its reason
. /usr/lib/lpoco-health/lib.sh
log=$(klog)
oops=$(printf '%s\n' "$log" | grep -cE 'Oops|BUG:|Kernel panic|Unable to handle kernel' || true)
check no-oops kernel "$([ "$oops" = 0 ] && echo true)" "$oops oops, BUG or panic lines this boot"
new=""; total=0
for w in $(printf '%s\n' "$log" | sed -n 's/.*WARNING: \([^ ]*\) at .*/\1/p'); do
	total=$((total + 1)); in_list "$w" "${KNOWN_WARNINGS:-}" || new="$new $w"
done
check boot-warnings kernel "$([ -z "$new" ] && echo true)" "$total warnings, ${new:+new:$new}${new:-all known}"
cr=$(printf '%s\n' "$log" | grep -E 'remoteproc.*(crash detected|fatal error)' | sed -n 's/.*crash detected in \([^:]*\).*/\1/p; s/.*remoteproc-\([a-z]*\).*fatal.*/\1/p' | sort | uniq -c | tr -s ' ' | tr '\n' ',')
check no-processor-crashes kernel "$([ -z "$cr" ] && echo true)" "${cr:-no signal processor crashed this boot}"
failed=""
for u in $(systemctl --failed --no-legend --plain 2>/dev/null | awk '{print $1}'); do
	[ "$u" = lpoco-health.service ] && continue
	in_list "$u" "${KNOWN_FAILED_UNITS:-}" || failed="$failed $u"
done
check no-failed-units kernel "$([ -z "$failed" ] && echo true)" "${failed:+failed:$failed}${failed:-no unit failed but the known: ${KNOWN_FAILED_UNITS:-none}}"
