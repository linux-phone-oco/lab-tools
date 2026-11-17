# the shell's sound: a sound server runs, and its default output is a real card - not a loopback or
# dummy (once, a loopback card became the default at boot and nothing was heard)
. /usr/lib/lpoco-health/lib.sh
[ -n "$(session_user)" ] || { say default-output audio-speaker skip "no graphical session: the base alone"; exit 0; }
sink=$(as_user pactl get-default-sink)
case $sink in
	"") say default-output audio-speaker fail "a session runs, but no sound server answers" ;;
	*aloop*|*dummy*|*null*|*Loopback*) say default-output audio-speaker fail "the default output is $sink" ;;
	*) say default-output audio-speaker pass "$sink" ;;
esac
