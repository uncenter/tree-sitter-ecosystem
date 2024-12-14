{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixpkgs-unstable";
    naersk.url = "github:nix-community/naersk";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      flake-utils,
      naersk,
      nixpkgs,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = (import nixpkgs) {
          inherit system;
        };

        naersk' = pkgs.callPackage naersk { };
      in
      {
        packages = {
          zeta = naersk'.buildPackage {
            pname = "zeta";
            src = ./zeta;
            nativeBuildInputs = with pkgs; [
              pkg-config
            ];
            buildInputs = with pkgs; [
              openssl
            ];
          };
        };

        devShell = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            clippy
            rustfmt
            rust-analyzer
          ];
          inputsFrom = [ self.packages.${system}.zeta ];
          env = {
            OPENSSL_NO_VENDOR = 1;
            RUST_SRC_PATH = toString pkgs.rustPlatform.rustLibSrc;
            RUST_LOG = "debug";
          };
        };
      }
    );
}
