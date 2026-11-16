# the other parts, as far as the kernel exposes them: NFC, USB role switch, headset jack, fingerprint
# reader, motion and light sensors, camera sensors - each absent part named in the manifest is skipped
. /usr/lib/devlab/health/lib.sh
part() { # part NAME TEST COMPONENT CONDITION DETAIL
	if absent "$1"; then say "$2" "$3" skip "the manifest says this phone has no $1"; else check "$2" "$3" "$4" "$5"; fi
}
nfc=$(ls /sys/class/nfc 2>/dev/null | tr '\n' ' ')
part nfc nfc-device nfc "$([ -n "$nfc" ] && echo true)" "${nfc:-no NFC device}"
r=$(ls /sys/class/usb_role 2>/dev/null | head -n1)
part usb-otg usb-role usb-otg "$([ -n "$r" ] && echo true)" "${r:-no USB role switch}"
jk=$(grep -c 'Headset Jack' /proc/bus/input/devices 2>/dev/null)
part jack jack-detector audio-headphone "$([ "${jk:-0}" -ge 1 ] && echo true)" "headset jack detectors: ${jk:-0} (sound through it is a witnessed test)"
fp=$(ls /dev /sys/bus/platform/devices 2>/dev/null | grep -ciE 'goodix|fingerprint|fpc1|_fp$')
part fingerprint reader-exposed fingerprint "$([ "${fp:-0}" -ge 1 ] && echo true)" "fingerprint reader devices: ${fp:-0}"
sn=0; for d in /sys/bus/iio/devices/iio:device*; do [ -e "$d/name" ] || continue; case "$(cat "$d/name")" in *adc*) ;; *) sn=$((sn + 1)) ;; esac; done
part sensors motion-and-light sensors "$([ "$sn" -ge 1 ] && echo true)" "motion, light or proximity sensors: $sn (voltage meters not counted)"
is=0; for d in /sys/class/video4linux/v4l-subdev*; do [ -e "$d/name" ] || continue; case "$(cat "$d/name")" in msm_*|*csiphy*|*csid*|*vfe*|*ispif*) ;; *) is=$((is + 1)) ;; esac; done
for c in camera-rear camera-front; do part camera image-sensor $c "$([ "$is" -ge 1 ] && echo true)" "image sensors bound: $is"; done
# the lab's own: on USB to a host, the phone holds a wake lock (devlab-usb-awake) - else every
# suspend attempt drops the link
if [ "$(cat /sys/class/udc/*/state 2>/dev/null | head -n1)" = configured ] && [ -x /usr/local/bin/devlab-usb-awake ]; then
	check usb-awake usb-otg "$(grep -q devlab-usb /sys/power/wake_lock 2>/dev/null && echo true)" "on USB to a host; wake locks: $(cat /sys/power/wake_lock 2>/dev/null)"
fi
