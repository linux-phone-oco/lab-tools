# SPDX-License-Identifier: MIT OR Apache-2.0
# sound cards: every card the manifest expects is registered (SOUND_CARDS: names, ;-separated)
. /usr/lib/lpoco-health/lib.sh
[ -n "${SOUND_CARDS:-}" ] || { say sound-card audio-speaker skip "the manifest names no sound card"; exit 0; }
echo "$SOUND_CARDS" | tr ';' '\n' | while read -r card; do
	[ -n "$card" ] || continue
	check sound-card audio-speaker "$(grep -qF "$card" /proc/asound/cards && echo true)" "'$card' in /proc/asound/cards: $(grep -c ']:' /proc/asound/cards) cards registered"
done
