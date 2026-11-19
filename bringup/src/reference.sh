# What a stock OS does for a part, read from the running phone: run as a shell script on the phone
# (lpoco-bringup reference capture --via "adb shell su 0 sh" sends it on standard input).
# Output: "@@dt PATH" then the property file as hex bytes, one section per property of /proc/device-tree;
# "@@file NAME" then the file's text, for what this phone lets us read. A file it does not give is left out.
cd "${LPOCO_DT:-/proc/device-tree}" || exit 1
find . -type f | sort | while read -r f; do
  echo "@@dt $f"
  od -An -v -tx1 "$f"
done
mount -t debugfs none /sys/kernel/debug 2>/dev/null
put() { # NAME FILE...
  n=$1; shift
  for f in "$@"; do
    if [ -r "$f" ]; then echo "@@file $n"; cat "$f"; return; fi
  done
}
put gpio /sys/kernel/debug/gpio
put regulator_summary /sys/kernel/debug/regulator/regulator_summary
put clk_summary /sys/kernel/debug/clk/clk_summary
put interrupts /proc/interrupts
if [ -r /sys/kernel/debug/pinctrl ]; then
  echo "@@file pinctrl"
  for f in /sys/kernel/debug/pinctrl/*/pinmux-pins; do [ -r "$f" ] && cat "$f"; done
fi
echo "@@file drivers"
for d in /sys/bus/*/drivers/*; do
  b=${d#/sys/bus/}; b=${b%%/*}
  for l in "$d"/*; do [ -L "$l" ] && echo "$b ${d##*/} ${l##*/}"; done
done
exit 0
