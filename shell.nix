# A reproducible development shell, without flakes: nix-shell. nixpkgs pinned by commit and hash (nixpkgs.nix),
# as in creation-guidelines/lab-agent.
let
  pkgs = import ./nixpkgs.nix;
in
pkgs.mkShell {
  packages = with pkgs; [ cargo rustc clippy rustfmt ];
}
