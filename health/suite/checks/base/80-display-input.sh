# display and input: a panel connected and lit, a touchscreen, a vibration motor
. /usr/lib/lpoco-health/lib.sh
if absent display; then say panel-connected display-panel skip "the manifest says this phone has no panel"; else
	conn=$(cat /sys/class/drm/card*-DSI*/status /sys/class/drm/card*-eDP*/status 2>/dev/null | head -n1)
	check panel-connected display-panel "$([ "$conn" = connected ] && echo true)" "panel: ${conn:-none}"
	b=$(cat /sys/class/backlight/*/brightness 2>/dev/null | head -n1)
	check backlight display-panel "$([ "${b:-0}" -gt 0 ] && echo true)" "brightness ${b:-none}"
fi
if absent touchscreen; then say touch-device touchscreen skip "the manifest says this phone has no touchscreen"; else
	t=$(grep -iE '^N: Name=.*(touch|ts|fts|nvt|goodix|synaptics|atmel)' /proc/bus/input/devices | head -n1 | sed 's/N: Name=//')
	check touch-device touchscreen "$([ -n "$t" ] && echo true)" "${t:-no touch input device}"
fi
# a vibration motor is an input device with force feedback: each device read as a whole block
if absent haptics; then say vibrator haptics skip "the manifest says this phone has no vibration motor"; else
	v=$(awk -v RS= '/\nB: FF=/' /proc/bus/input/devices | sed -n 's/^N: Name=//p' | head -n1)
	check vibrator haptics "$([ -n "$v" ] && echo true)" "${v:-no force-feedback input device}"
fi
