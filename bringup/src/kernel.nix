# SPDX-License-Identifier: MIT OR Apache-2.0
# One kernel file of `[kernel_builds.NAME]` as a fixed-output derivation: the int/ tag's commit built by the pinned
# nixpkgs' clang, with the clock, user, host and build counter fixed, so the same declaration gives the same bytes on
# any machine and Nix refuses a result whose sha256 is not the declared one (outputHash). Everything it reads is a
# store path (the config fragments, the patches) or the commit named by `rev`. Written beside nixpkgs.nix by `image ensure`.
{ name, sha256, repo, tag, rev, epoch, defconfig, targets, output
, configs ? "", patches ? "", dtcFlags ? "" }:
let
  pkgs = import ./nixpkgs.nix;
  lib = pkgs.lib;
  words = s: lib.filter (w: w != "") (lib.splitString " " s);
  # an input is a store path: storePath puts it in the derivation's inputs, a bare string would not
  stores = s: map builtins.storePath (words s);
  # the tag's commit, fetched by the tag (a commit on no branch is not offered) and held to `rev`
  src = builtins.fetchGit {
    url = repo;
    ref = "refs/tags/${tag}";
    inherit rev;
    shallow = true;
  };
in
pkgs.llvmPackages.stdenv.mkDerivation {
  inherit name;
  nativeBuildInputs = with pkgs; [
    llvmPackages.lld
    llvmPackages.llvm
    gnumake
    bison
    flex
    bc
    perl
    python3Minimal
    cpio
    xz
    gzip
    zstd
    lz4
    patch
    gawk
    rsync
    which
  ];
  # libraries the kernel's host tools run against: buildInputs, so the wrapper gives them an rpath
  buildInputs = [ pkgs.openssl pkgs.elfutils ];
  hardeningDisable = [ "all" ];
  # the build's own clock and names: what made the hand-built kernels differ (research image-recipes-how-it-stands, 4)
  KBUILD_BUILD_USER = "lpoco";
  KBUILD_BUILD_HOST = "lab";
  KBUILD_BUILD_VERSION = "1";
  LD_LIBRARY_PATH = "${pkgs.openssl.out}/lib";
  DTC_FLAGS = dtcFlags;
  SOURCE_DATE_EPOCH = toString epoch;
  buildCommand = ''
    export KBUILD_BUILD_TIMESTAMP="$(date -u -d @${toString epoch})"
    cp -r ${src} tree
    chmod -R u+w tree
    ${lib.concatMapStringsSep "\n" (p: "patch -p1 -d tree < ${p}") (stores patches)}
    # the target is compiled by the unwrapped clang (the wrapper cannot cross-compile); the wrapped one builds the host tools
    make="make -C tree ARCH=arm64 LLVM=1 CC=${pkgs.llvmPackages.clang-unwrapped}/bin/clang O=$PWD/b KCFLAGS=-Wno-unused-command-line-argument HOSTCFLAGS=-Wno-unused-command-line-argument HOSTCXXFLAGS=-Wno-unused-command-line-argument"
    $make ${defconfig}
    tree/scripts/kconfig/merge_config.sh -m -O b b/.config ${lib.concatStringsSep " " (stores configs)}
    $make olddefconfig
    $make -j$NIX_BUILD_CORES ${lib.concatStringsSep " " (words targets)}
    cp b/${output} $out
  '';
  outputHashMode = "flat";
  outputHashAlgo = "sha256";
  outputHash = sha256;
}
