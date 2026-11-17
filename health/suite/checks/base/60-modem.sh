# the modem: seen by ModemManager; with a SIM, registered (without one, calls, messages and data
# cannot be shown - unknown, not broken)
. /usr/lib/devlab/health/lib.sh
absent modem && { say modem-seen modem-data skip "the manifest says this phone has no modem"; exit 0; }
command -v mmcli >/dev/null || { say modem-seen modem-data skip "no ModemManager on the phone"; exit 0; }
m=""; i=0
while [ $i -lt 6 ]; do m=$(mmcli -L 2>/dev/null | grep -o '/Modem/[0-9]*' | head -n1); [ -n "$m" ] && break; i=$((i + 1)); sleep 5; done
check modem-seen modem-data "$([ -n "$m" ] && echo true)" "ModemManager: ${m:-no modem after 30 s}"
[ -n "$m" ] || exit 0
info=$(mmcli -m "${m##*/}" 2>/dev/null | sed 's/\x1b\[[0-9;]*m//g')
mst=$(printf '%s\n' "$info" | awk -F': ' '/ state:/ && !/power/ {print $2; exit}')
why=$(printf '%s\n' "$info" | awk -F': ' '/failed reason:/ {print $2; exit}')
if printf '%s\n' "$info" | grep -q '/SIM/'; then
	check modem-registered modem-data "$(case "$mst" in registered|connected|enabled|connecting) echo true;; esac)" "modem state: ${mst:-unknown}"
else say modem-registered modem-data skip "no SIM card (modem state: ${mst:-unknown}${why:+, $why})"; fi
