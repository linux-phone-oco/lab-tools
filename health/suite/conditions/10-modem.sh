# SPDX-License-Identifier: MIT OR Apache-2.0
# Detected conditions are worked out on the phone at every run, never typed into docs or settings: a line
# "NAME detail" says a condition holds right now; what needs it says "skip: detail" - unknown, not broken.
. /usr/lib/lpoco-health/lib.sh
absent modem && exit 0
command -v mmcli >/dev/null || { echo "no-modem no ModemManager"; exit 0; }
mmcli -L 2>/dev/null | grep -q '/Modem/' || { echo "no-modem no modem seen by ModemManager"; exit 0; }
i=$(mmcli -m any -K 2>/dev/null) || exit 0
[ -n "$i" ] || exit 0
# ModemManager: state failed with reason sim-missing, or no SIM object at all
if printf '%s\n' "$i" | grep -q 'state-failed-reason *: *sim-missing' ||
	printf '%s\n' "$i" | grep -q '^modem\.generic\.sim *: *\(--\)\{0,1\}$'; then
	echo "no-sim no SIM card"
fi
