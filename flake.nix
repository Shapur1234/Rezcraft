{
  description = "Rezcraft";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # The version of wasm-bindgen-cli needs to match the version in Cargo.lock
    nixpkgs-for-wasm-bindgen.url = "github:NixOS/nixpkgs/75c13bf6aac049d5fec26c07c28389a72c25a30b";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs = {
        nixpkgs.follows = "nixpkgs";
        flake-utils.follows = "flake-utils";
      };
    };
  };

  outputs = { self, nixpkgs, crane, flake-utils, rust-overlay, nixpkgs-for-wasm-bindgen, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ (import rust-overlay) ];
        };

        inherit (pkgs) lib;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          targets = [
            "wasm32-unknown-unknown"
            "x86_64-pc-windows-gnu"
          ];
          extensions = [ "rust-src" ];
        };
        craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain).overrideScope' (_final: _prev: {
          inherit (import nixpkgs-for-wasm-bindgen { inherit system; }) wasm-bindgen-cli;
        });

        src = craneLib.cleanCargoSource (craneLib.path ./.);

        runtimeLibs = with pkgs; [
          vulkan-loader

          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr

          wayland
          wayland-protocols
        ];
        LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;

        commonArgs = {
          inherit src;
          strictDeps = true;

          pname = "rezcraft";
          version = "0.1.0";

          nativeBuildInputs = with pkgs; [
            makeWrapper
          ];
          buildInputs = [
            runtimeLibs
          ] ++ lib.optionals pkgs.stdenv.isDarwin [
            pkgs.libiconv
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
        };
        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        rezcraft = craneLib.buildPackage (commonArgs // {
          inherit cargoArtifacts;

          postInstall = ''
            wrapProgram "$out/bin/rezcraft" --set LD_LIBRARY_PATH ${lib.makeLibraryPath runtimeLibs};
            cp -r ./res/ $out/bin/
          '';
        });
      in
      {
        checks = {
          my-crate = rezcraft;

          my-crate-clippy = craneLib.cargoClippy (commonArgs // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          my-crate-fmt = craneLib.cargoFmt {
            inherit src;
          };
        };

        packages = {
          default = rezcraft;
        };

        apps.default = flake-utils.lib.mkApp {
          drv = rezcraft;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs;[
            rustToolchain
            runtimeLibs

            cargo-flamegraph
            cargo-outdated
            gdb
            rustup
            sfz
            wasm-pack
          ];

          inherit LD_LIBRARY_PATH;
        };
      });
}
