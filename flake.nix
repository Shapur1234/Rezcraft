{
  description = "Rezcraft";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # The version of wasm-bindgen-cli needs to match the version in Cargo.lock
    nixpkgs-for-wasm-bindgen.url = "github:NixOS/nixpkgs/5eb7b63c5c2d02ad4711d8dff1d824ac39f2cc3a";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
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

        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "rust-std" "rust-src" ];
          targets = [ "wasm32-unknown-unknown" "x86_64-pc-windows-gnu" ];
        });

        craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain).overrideScope (_final: _prev: {
          inherit (import nixpkgs-for-wasm-bindgen { inherit system; }) wasm-bindgen-cli;
        });

        src = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            (lib.hasSuffix "\.html" path) ||
            (lib.hasInfix "/res/" path) ||
            (craneLib.filterCargoSources path type)
          ;
        };

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
          version = "0.2.0";
        };

        nativeArgs = commonArgs // {
          cargoExtraArgs = "--no-default-features --features rayon,save_system";

          buildInputs = [
            runtimeLibs
          ];
          nativeBuildInputs = with pkgs; [
            rename
            makeWrapper
          ];

          inherit LD_LIBRARY_PATH;
        };
        wasmArgs = commonArgs // {
          cargoExtraArgs = "--no-default-features --features portable";
          pname = "rezcraft-wasm";

          doCheck = false;

          TRUNK_BUILD_MINIFY = "always";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
        };

        nativeCargoArtifacts = craneLib.buildDepsOnly nativeArgs;
        wasmCargoArtifacts = craneLib.buildDepsOnly wasmArgs;

        nativeCrate = craneLib.buildPackage (nativeArgs // {
          cargoArtifacts = nativeCargoArtifacts;

          postInstall = ''
            wrapProgram "$out/bin/rezcraft" --set LD_LIBRARY_PATH ${lib.makeLibraryPath runtimeLibs}
            cp -r ./res/ $out/bin/
          '';
        });
        winCrate = craneLib.buildPackage (nativeArgs // {
          doCheck = false;

          CARGO_BUILD_TARGET = "x86_64-pc-windows-gnu";

          depsBuildBuild = with pkgs; [
            pkgsCross.mingwW64.stdenv.cc
          ];
          CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS = "-L native=${pkgs.pkgsCross.mingwW64.windows.pthreads}/lib";
        });
        wasmCrate = craneLib.buildTrunkPackage (wasmArgs // {
          cargoArtifacts = wasmCargoArtifacts;
          wasm-bindgen-cli = pkgs.wasm-bindgen-cli.override {
            version = "0.2.92";
            hash = "sha256-1VwY8vQy7soKEgbki4LD+v259751kKxSxmo/gqE6yV0=";
            cargoHash = "sha256-aACJ+lYNEU8FFBs158G1/JG8sc6Rq080PeKCMnwdpH0=";
          };
        });

        serve-wasm = pkgs.writeShellScriptBin "${wasmArgs.pname}" ''
          ${pkgs.sfz}/bin/sfz ${wasmCrate} -r --coi'';

        nativeCrateClippy = craneLib.cargoClippy (nativeArgs // {
          inherit src;
          cargoArtifacts = nativeCargoArtifacts;

          # cargoClippyExtraArgs = "-- --deny warnings";
        });
      in
      {
        checks = {
          inherit nativeCrate;
          inherit winCrate;
          inherit wasmCrate;

          inherit nativeCrateClippy;

          fmt = craneLib.cargoFmt commonArgs;
        };

        packages = {
          default = nativeCrate;

          rezcraft-native = nativeCrate;
          rezcraft-win = winCrate;
          rezcraft-wasm = wasmCrate;
        };

        apps = {
          default = flake-utils.lib.mkApp {
            name = "rezcraft-native";
            drv = nativeCrate;
          };

          rezcraft-native = flake-utils.lib.mkApp {
            name = "rezcraft-native";
            drv = nativeCrate;
          };
          rezcraft-wasm = flake-utils.lib.mkApp {
            name = "rezcraft-wasm";
            drv = serve-wasm;
          };
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs;[
            rustToolchain
            runtimeLibs

            cargo-flamegraph
            cargo-outdated
            gdb

            trunk
            sfz
          ];

          inherit LD_LIBRARY_PATH;
        };
      });
}
