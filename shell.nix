# A reproducible development shell, without flakes: nix-shell. nixpkgs pinned by commit and hash,
# as in creation-guidelines/lab-agent.
let
  pkgs = import (fetchTarball {
    # nixos-26.05
    url = "https://github.com/NixOS/nixpkgs/archive/c56a87d8e41ce69bd2af57eb35dcc117a9dfca33.tar.gz";
    sha256 = "06g8y8y9bysnpxrl6bfqmn63l8w2lx2yhj0618r34j5jfqy5vjpx";
  }) { };
in
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc clippy rustfmt ];
}
