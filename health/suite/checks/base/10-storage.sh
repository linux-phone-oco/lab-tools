# SPDX-License-Identifier: MIT OR Apache-2.0
# storage: the system fills its partition (a first boot can grow the partition and leave the
# filesystem at the image's size), has room, and can be written
. /usr/lib/lpoco-health/lib.sh
src=$(findmnt -no SOURCE /); fs=$(df -B1 --output=size / | tail -n1 | tr -d ' ')
dev=$(blockdev --getsize64 "$src" 2>/dev/null || echo 0)
if [ "$dev" -gt 0 ]; then
	check root-fills-its-partition kernel "$([ $((fs * 100 / dev)) -ge 90 ] && echo true)" "the system is $((fs >> 30)) GB on a partition of $((dev >> 30)) GB"
else say root-fills-its-partition kernel skip "the partition under / ($src) could not be measured"; fi
use=$(df --output=pcent / | tail -n1 | tr -dc 0-9)
check root-has-room kernel "$([ "${use:-100}" -lt 90 ] && echo true)" "/ is ${use}% full"
f=/var/tmp/.lpoco-health-write-test
check root-writable kernel "$(echo ok > $f 2>/dev/null && [ "$(cat $f)" = ok ] && rm -f $f && echo true)" "writing a file under /var"
