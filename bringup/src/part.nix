# SPDX-License-Identifier: MIT OR Apache-2.0
# One binary of `[part_builds.NAME]` as a fixed-output derivation: the crate at a commit, its unit tests run first,
# then built for aarch64-unknown-linux-gnu with the C library linked static (+crt-static) so it runs in a ramdisk with
# no libraries. The std library is built from the pinned toolchain's own source (-Zbuild-std), the crates from the
# vendored Cargo.lock, the paths remapped: the same declaration gives the same bytes on any machine, and Nix refuses
# a result whose sha256 is not the declared one (outputHash). With `nixpkgs` set the part is instead one file of a
# package of the pinned nixpkgs, built static for aarch64 (busybox). With `rebuild` set it is held to nothing: `image ensure
# --rebuild` compares the result itself. Written beside nixpkgs.nix by `image ensure`.
{ name, sha256, repo ? null, rev ? null, crate ? ".", nixpkgs ? null, bin, rebuild ? null }:
let
  pkgs = import ./nixpkgs.nix;
  cross = pkgs.pkgsCross.aarch64-multiplatform;
  src = builtins.fetchGit {
    url = repo;
    inherit rev;
    allRefs = true;
  };
  dir = if crate == "." then src else src + "/${crate}";
  # the crates of the crate's lock, and those of the std library's own (build-std resolves both)
  vendored = lock: pkgs.rustPlatform.importCargoLock {
    lockFile = lock + "/Cargo.lock";
    allowBuiltinFetchGit = true;
  };
  libSrc = pkgs.rustPlatform.rustLibSrc;
  # held to the declared sha256, or to nothing when `image ensure --rebuild` compares the result itself
  held = if rebuild != null then { } else {
    outputHashMode = "flat";
    outputHashAlgo = "sha256";
    outputHash = sha256;
  };
  # a program that is not a crate (busybox): the pinned nixpkgs' own static aarch64 build of it, one file of its bin/,
  # its one reference to itself overwritten as below
  packaged = pkgs.runCommand name held ''
    install -m 0644 ${cross.pkgsStatic.${nixpkgs}}/bin/${bin} $out
    LC_ALL=C sed -i -E 's|/nix/store/[a-z0-9]{32}-|/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-|g' $out
  '';
in
if nixpkgs != null then packaged else
pkgs.stdenv.mkDerivation ({
  inherit name;
  nativeBuildInputs = [ pkgs.cargo pkgs.rustc cross.stdenv.cc ];
  buildInputs = [ cross.glibc.static ];
  buildCommand = ''
    cp -r ${dir} crate
    chmod -R u+w crate
    mkdir vendor
    cp -r ${vendored dir}/. vendor/
    chmod -R u+w vendor
    cp -rn ${vendored libSrc}/. vendor/
    mkdir -p crate/.cargo
    cat > crate/.cargo/config.toml <<EOF
    [source.crates-io]
    replace-with = "vendored-sources"
    [source.vendored-sources]
    directory = "$PWD/vendor"
    EOF
    # build-std is a nightly flag (the build line sets RUSTC_BOOTSTRAP for it alone); the std source is read from this root
    export __CARGO_TESTS_ONLY_SRC_ROOT=${libSrc}
    export CARGO_HOME=$PWD/cargo-home
    cd crate
    # the unit tests run first, on this machine, and stop the build when one fails
    cargo test --locked --offline --quiet
    CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-unknown-linux-gnu-gcc \
    RUSTC_BOOTSTRAP=1 RUSTFLAGS="-C target-feature=+crt-static --remap-path-prefix=$NIX_BUILD_TOP=/build --remap-path-prefix=${pkgs.rustc}=/rustc --remap-path-prefix=${libSrc}=/rustc-src" \
      cargo build --locked --offline --release --target aarch64-unknown-linux-gnu -Zbuild-std=std,panic_abort --target-dir $NIX_BUILD_TOP/target
    # a flat fixed-output file is not executable: the ramdisk's builder gives the mode
    install -m 0644 $NIX_BUILD_TOP/target/aarch64-unknown-linux-gnu/release/${bin} $out
    # a static glibc and libgcc carry the store paths they were built at (locale, debug info), and a fixed-output file
    # may not refer to the store: the hashes are overwritten in place, same length, as nixpkgs' remove-references-to does
    LC_ALL=C sed -i -E 's|/nix/store/[a-z0-9]{32}-|/nix/store/eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee-|g' $out
  '';
} // held)
