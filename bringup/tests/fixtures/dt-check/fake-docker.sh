#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# a stand-in for docker: records its arguments, then plays what the container would say.
# FAKE_DOCKER_LOG: where the arguments go (one per line).
# A source tree holding BOGUS makes it fail like dtschema (violation, exit 2); holding WARN it reports the
# same violation as a warning and exits 0, as CHECK_DTBS does; holding pm.dts it reports a violation only
# when the node is enabled (dtschema skips disabled nodes).
printf '%s\n' "$@" >"$FAKE_DOCKER_LOG"
[ -n "$FAKE_DOCKER_EXIT" ] && exit "$FAKE_DOCKER_EXIT"
for a in "$@"; do
  case "$a" in
    *:/src:ro)
      src=${a%%:/src:ro}
      if [ -f "$src/BOGUS" ]; then
        echo "arch/arm64/boot/dts/qcom/x.dtb: pmic: 'bogus-prop' does not match any of the regexes: Unevaluated properties are not allowed" >&2
        exit 2
      fi
      if [ -f "$src/WARN" ]; then
        echo "arch/arm64/boot/dts/qcom/x.dtb: pmic: 'bogus-prop' does not match any of the regexes: Unevaluated properties are not allowed" >&2
        exit 0
      fi
      if [ -f "$src/pm.dts" ] && ! grep -q 'status = "disabled"' "$src/pm.dts"; then
        echo "arch/arm64/boot/dts/qcom/pm8998.dtb: pmic@0: 'bogus-prop' does not match any of the regexes: Unevaluated properties are not allowed" >&2
        exit 0
      fi
      if [ -f "$src/SETUP" ]; then
        echo "make: *** No rule to make target 'dtc'.  Stop." >&2
        exit 2
      fi ;;
  esac
done
echo "  DTC_CHK qcom/x.dtb"
exit 0
