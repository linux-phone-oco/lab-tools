#!/bin/sh
# SPDX-License-Identifier: MIT OR Apache-2.0
# a stand-in for docker: records its arguments, then plays what the container would say.
# FAKE_DOCKER_LOG: where the arguments go (one per line); a source tree holding bogus-prop.yaml-error makes it fail like dtschema.
printf '%s\n' "$@" >"$FAKE_DOCKER_LOG"
[ -n "$FAKE_DOCKER_EXIT" ] && exit "$FAKE_DOCKER_EXIT"
for a in "$@"; do
  case "$a" in
    *:/src:ro)
      src=${a%%:/src:ro}
      if [ -f "$src/BOGUS" ]; then
        echo "arch/arm64/boot/dts/qcom/x.dtb: pmic: 'bogus-prop' does not match any of the regexes: Unevaluated properties are not allowed" >&2
        exit 2
      fi ;;
  esac
done
echo "  DTC_CHK qcom/x.dtb"
exit 0
