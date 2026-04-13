{
  description = "rust flake sample";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      nixpkgs,
      rust-overlay,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.stable.latest.default;
        rustPlatform = pkgs.makeRustPlatform {
          rustc = toolchain;
          cargo = toolchain;
        };
      in
      {
        devShells.default =
          with pkgs;
          mkShell {
            buildInputs = [
              openssl
              pkg-config
              (rust-bin.stable.latest.default.override { extensions = [ "rust-src" ]; })
            ];
            RUST_SRC_PATH = "${pkgs.rust.packages.stable.rustPlatform.rustLibSrc}";
          };
        packages =
          let
            biwac = rustPlatform.buildRustPackage {
              pname = "biwac";
              version = "0.1.0";
              src = ./.;
              cargoLock.lockFile = ./Cargo.lock;
              doCheck = false;
            };
          in
          {
            sample = pkgs.stdenv.mkDerivation {
              name = "biwa-example";
              src = ./assets/tests/test1;

              nativeBuildInputs = [ biwac ];

              buildPhase = ''
                mkdir -p .biwa_build
                cp biwa-dependencies.json .biwa_build/
                biwac
              '';

              installPhase = ''
                mkdir -p $out/.biwa_build/
                cp .biwa_build/typescript/src/generated/*.ts $out/.biwa_build/
              '';
            };
          };
      }
    );
}

