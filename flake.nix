{
  description = "Simple Rust dev shell";

  inputs = { nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable"; };

  outputs = { self, nixpkgs }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
    in {
      devShells.${system}.default = pkgs.mkShell {
        buildInputs = with pkgs; [
          rustc
          cargo
          rustfmt
          clippy
          pkg-config
          openssl
          gcc
          libiconv
          openxr-loader
          rust-analyzer
        ];
      };
    };
}
