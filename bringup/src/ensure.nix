# SPDX-License-Identifier: MIT OR Apache-2.0
# One image of a recipe as a fixed-output derivation: the control image with parts swapped in by
# `lpoco-bringup image swap`, and Nix refuses a result whose sha256 is not the declared one (outputHash).
# Everything it reads is a store path (the control, the parts) or the pinned nixpkgs; no network is needed
# beyond what building lpoco-bringup from `source` takes. Written beside nixpkgs.nix by `image ensure`.
{ name, sha256, control, source ? null, version, kernel ? null, dtb ? null, ramdisk ? null, cmdline ? null }:
let
  pkgs = import ./nixpkgs.nix;
  src =
    if source != null then
      builtins.path {
        path = source;
        name = "lab-tools-source";
        filter = path: type: !(builtins.elem (baseNameOf path) [ "target" ".git" ]);
      }
    else
      builtins.fetchTarball "https://github.com/linux-phone-oco/lab-tools/archive/refs/tags/lpoco-v${version}.tar.gz";
  bringup = pkgs.rustPlatform.buildRustPackage {
    pname = "lpoco-bringup";
    inherit version src;
    # the workspace lock holds git dependencies (lpoco-attended's): fetched by Nix's own fetchGit, pinned by the lock's revs
    cargoLock = {
      lockFile = src + "/Cargo.lock";
      allowBuiltinFetchGit = true;
    };
    cargoBuildFlags = [ "-p" "lpoco-bringup" ];
    doCheck = false;
  };
  # avbtool 1.4.0 as the lab's host has it (sha256 1aa6f2b4...): nixpkgs' android-tools writes avbtool 1.3.0, whose
  # footer differs by one byte (measured 2026-11-23). The source is the AOSP revision android-tools 37.0.0p1 vendors,
  # until image-avb-footer-in-rust removes avbtool from the recipe.
  avb = builtins.fetchGit {
    url = "https://android.googlesource.com/platform/external/avb";
    rev = "ba2dec4b035b0a3b61c5f8f8a74d86bcd450b1ee";
  };
  avbtool = pkgs.writeShellScriptBin "avbtool" ''exec ${pkgs.python3}/bin/python3 ${avb}/avbtool.py "$@"'';
  part = flag: p: pkgs.lib.optionals (p != null) [ flag (builtins.storePath p) ];
in
derivation {
  inherit name;
  system = builtins.currentSystem;
  builder = "${bringup}/bin/lpoco-bringup";
  args =
    [ "image" "swap" (builtins.storePath control) "--out" (placeholder "out") ]
    ++ part "--kernel" kernel
    ++ part "--dtb" dtb
    ++ part "--ramdisk" ramdisk
    ++ pkgs.lib.optionals (cmdline != null) [ "--cmdline" cmdline ];
  # the AVB footer is still avbtool's (image-avb-footer-in-rust removes it)
  PATH = "${avbtool}/bin";
  outputHashMode = "flat";
  outputHashAlgo = "sha256";
  outputHash = sha256;
}
