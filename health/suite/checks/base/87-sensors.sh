# SPDX-License-Identifier: MIT OR Apache-2.0
# motion, light and proximity: exposed to the system, and proximity giving a reading. Sensors reach Linux
# two ways - as IIO devices the kernel drives, or through a sensor hub on a DSP (Qualcomm's Sensor Core,
# served by hexagonrpcd and read by libssc) that only iio-sensor-proxy shows. Both are counted, through
# iio-sensor-proxy's own answer (net.hadess.SensorProxy) where it runs. The proximity sensor is the one a
# call needs: it turns the screen off at the ear. A reading proves it present and live; near and far
# when covered is a witnessed test.
. /usr/lib/lpoco-health/lib.sh
if absent sensors; then say motion-and-light sensors skip "the manifest says this phone has no sensors"; exit 0; fi
sn=0; for d in /sys/bus/iio/devices/iio:device*; do [ -e "$d/name" ] || continue; case "$(cat "$d/name")" in *adc*) ;; *) sn=$((sn + 1)) ;; esac; done
has() { busctl --system get-property net.hadess.SensorProxy /net/hadess/SensorProxy net.hadess.SensorProxy "Has$1" 2>/dev/null | grep -q true && echo "$1"; }
px=""; for s in Accelerometer AmbientLight Proximity Compass; do px="$px $(has $s)"; done
px=$(echo $px)
check motion-and-light sensors "$([ "$sn" -ge 1 ] || [ -n "$px" ] && echo true)" \
	"IIO sensors: $sn (voltage meters not counted); through iio-sensor-proxy: ${px:-none$(systemctl is-active -q iio-sensor-proxy 2>/dev/null && echo ', the proxy runs but serves no sensor')}"
if absent proximity; then say proximity-reads sensors skip "the manifest says this phone has no proximity sensor"; exit 0; fi
case " $px " in
*" Proximity "*)
	r=$(timeout 8 monitor-sensor --proximity 2>/dev/null | grep -m1 -o 'near: [01]\|Proximity value changed: [01]')
	check proximity-reads sensors "$([ -n "$r" ] && echo true)" "proximity sensor through iio-sensor-proxy: ${r:-no reading in 8 s}" ;;
*) say proximity-reads sensors fail "no proximity sensor exposed (iio-sensor-proxy has none)" ;;
esac
