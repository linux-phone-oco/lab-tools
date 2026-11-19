# SPDX-License-Identifier: MIT OR Apache-2.0
# A lab phone does not idle-suspend: sxmo's autosuspend loop (sxmo_autosuspend.sh -> rtcwake -m mem) drops
# USB and Wi-Fi and the phone looks hung. It cannot suspend while the kernel wake lock `lpoco-awake` is held,
# and the unit holding it (lpoco-awake.service, from this package: lpoco-health awake) must run.
# Reads only. skip: autosleep is deliberately allowed (/etc/lpoco/allow-autosleep), where the lock follows the
# USB port and the gates the manifest names - the unit being active is then all that is asked.
. "${LPOCO_HEALTH_LIB:-/usr/lib/lpoco-health/lib.sh}"
act=$(systemctl is-active lpoco-awake.service 2>/dev/null)
if [ -e /etc/lpoco/allow-autosleep ]; then
	check awake-service-active deep-sleep "$([ "$act" = active ] && echo true)" "lpoco-awake.service: ${act:-unknown}"
	say awake-wakelock-held deep-sleep skip "/etc/lpoco/allow-autosleep present: idle suspend allowed on purpose"
	exit 0
fi
held=$(grep -qE "(^| )lpoco-awake( |$)" /sys/power/wake_lock 2>/dev/null && echo true)
check awake-wakelock-held deep-sleep "$held" "wake lock lpoco-awake in /sys/power/wake_lock: $(cat /sys/power/wake_lock 2>&1)"
check awake-service-active deep-sleep "$([ "$act" = active ] && echo true)" "lpoco-awake.service: ${act:-unknown}"
