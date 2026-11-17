#!/bin/sh
# Build lpoco-health, the runner that goes onto phones: static, for the phone's architecture.
#   health/build.sh aarch64-unknown-linux-musl
# On the lab machine the standard library for the target comes from a toolchain kept beside the
# system's (rustup, in ~/.local/share/devlab-cross) and the lab's aarch64 GCC links it; in the
# package conveyor's CI the target is added by rustup as for every other binary it builds.
set -eu
target="${1:?usage: $0 TARGET - e.g. aarch64-unknown-linux-musl}"
cd "$(dirname "$0")/.."
if [ -d "$HOME/.local/share/devlab-cross/rustup" ] && [ ! -d "$(rustc --print target-libdir --target "$target" 2>/dev/null)" ]; then
	export RUSTUP_HOME="$HOME/.local/share/devlab-cross/rustup" CARGO_HOME="$HOME/.local/share/devlab-cross/cargo"
	exec nix-shell -p rustup --run "CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc cargo +$(rustc -V | cut -d' ' -f2) build --release --locked -p lpoco-health --target $target && echo built target/$target/release/lpoco-health"
fi
cargo build --release --locked -p lpoco-health --target "$target"
echo "built target/$target/release/lpoco-health"
