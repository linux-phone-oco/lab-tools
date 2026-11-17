# mobile data carries traffic, on any phone with a modem: over the modem's own interface - the one
# ModemManager's connected bearer names, so Wi-Fi cannot pass for it - five pings to 1.1.1.1 and an
# HTTPS download of 100 kB, or 5 MB once a week to measure throughput. What the run moved is read off
# the interface's own counters and kept in the month's ledger (devlab-health data); once the month has
# spent the manifest's budget (DATA_BUDGET_MB, 250 by default) the check skips, as it does without a SIM
# or a registration: what nobody measured is unknown, not broken. About 150 kB a boot.
. /usr/lib/devlab/health/lib.sh
absent modem && { say mobile-data modem-data skip "the manifest says this phone has no modem"; exit 0; }
command -v mmcli >/dev/null || { say mobile-data modem-data skip "no ModemManager"; exit 0; }
m=$(mmcli -L 2>/dev/null | grep -o '/Modem/[0-9]*' | head -n1)
[ -n "$m" ] || { say mobile-data modem-data skip "no modem seen by ModemManager"; exit 0; }
no_sim && { say mobile-data modem-data skip "no SIM card"; exit 0; }
# KEY's value in mmcli's key-value output on standard input
kv() { sed -n "s/^$1 *: *//p" | head -n1; }
info=$(mmcli -m "${m##*/}" -K 2>/dev/null)
state=$(printf '%s\n' "$info" | kv 'modem\.generic\.state')
tech=$(printf '%s\n' "$info" | kv 'modem\.generic\.access-technologies\.value\[1\]')
case $state in
	registered|connected|connecting) ;;
	*) say mobile-data modem-data skip "modem state ${state:-unknown}: no registration to carry data"; exit 0 ;;
esac
budget=${DATA_BUDGET_MB:-250}
plan=$(devlab-health data plan --budget-mb "$budget")
mb() { echo "$(( ${1:-0} / 1000000 )).$(( ${1:-0} % 1000000 / 100000 ))"; }
if [ "$(json_field over "$plan")" = true ]; then
	say mobile-data modem-data skip "the month's budget is spent: $(mb "$(json_field used_bytes "$plan")") of $budget MB in $(json_field month "$plan")"
	exit 0
fi
# the interface of the bearer that carries data: connected, not the IMS bearer a modem may hold beside it
# for VoLTE and SMS (APN "ims", no route of its own), the one with a default route when there are several
bearer_iface() {
	first=""
	for b in $(mmcli -m "${m##*/}" -K 2>/dev/null | sed -n 's/^modem\.generic\.bearers\.value\[[0-9]*\] *: *//p'); do
		bi=$(mmcli -b "${b##*/}" -K 2>/dev/null)
		[ "$(printf '%s\n' "$bi" | kv 'bearer\.status\.connected')" = yes ] || continue
		case "$(printf '%s\n' "$bi" | kv 'bearer\.properties\.apn')" in [Ii][Mm][Ss]) continue ;; esac
		i=$(printf '%s\n' "$bi" | kv 'bearer\.status\.interface')
		[ -n "$i" ] || continue
		if { ip route; ip -6 route; } 2>/dev/null | grep -q "^default .*dev $i "; then echo "$i"; return; fi
		[ -n "$first" ] || first=$i
	done
	[ -z "$first" ] || echo "$first"
}
iface=$(bearer_iface)
if [ -z "$iface" ]; then
	# registered but not connected: ask NetworkManager to bring the modem's connection up, once
	dev=$(nmcli -t -f DEVICE,TYPE device 2>/dev/null | awk -F: '$2=="gsm"{print $1; exit}')
	[ -n "$dev" ] && nmcli device connect "$dev" >/dev/null 2>&1 && sleep 3
	iface=$(bearer_iface)
fi
[ -n "$iface" ] && [ -d "/sys/class/net/$iface" ] ||
	{ say mobile-data modem-data fail "registered ($state), but no bearer connected: the mobile connection did not come up"; exit 0; }
moved() { echo $(( $(cat "/sys/class/net/$iface/statistics/rx_bytes") + $(cat "/sys/class/net/$iface/statistics/tx_bytes") )); }
size=$(json_field download_bytes "$plan")
weekly=$(json_field weekly "$plan")
before=$(moved)
pings=$(ping -c5 -W3 -I "$iface" 1.1.1.1 2>/dev/null)
got=$(printf '%s\n' "$pings" | sed -n 's/.*, \([0-9]*\) \(packets \)\{0,1\}received.*/\1/p')
rtt=$(printf '%s\n' "$pings" | sed -n 's|.* = [0-9.]*/\([0-9.]*\)/.*|\1|p')
dl=$(curl -s -o /dev/null --max-time 60 --interface "$iface" -w "%{size_download} %{speed_download} %{time_namelookup}" \
	"https://speed.cloudflare.com/__down?bytes=${size:-100000}")
after=$(moved)
used=$(( after - before )); [ "$used" -ge 0 ] || used=0
set -- $dl
# the week's run is marked made only when its download came whole: one cut short is tried again next boot
ledger=$(devlab-health data spent "$used" --budget-mb "$budget" $([ "$weekly" = true ] && [ "${1:-0}" -ge "${size:-0}" ] && echo --weekly))
check mobile-data modem-data "$([ "${got:-0}" -ge 3 ] && [ "${1:-0}" -ge "${size:-100000}" ] && echo true)" \
	"over $iface ($state, ${tech:-?}): ${got:-0} of 5 pings answered${rtt:+ in $rtt ms}, ${1:-0} bytes downloaded at ${2:-0} B/s (DNS ${3:-?} s)$([ "$weekly" = true ] && echo ', the week'"'"'s throughput run'); the run moved $used bytes, the month $(mb "$(json_field used_bytes "$ledger")") of $budget MB"
