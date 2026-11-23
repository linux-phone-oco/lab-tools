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
  outputHashMode = "flat";
  outputHashAlgo = "sha256";
  outputHash = sha256;
}
