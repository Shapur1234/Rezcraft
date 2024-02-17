{
  description = "Rezcraft";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    # wasm-bindgen-cli 0.2.91
    nixpkgs-for-wasm-bindgen.url = "github:NixOS/nixpkgs/38513315386e828b9d296805657726e63e338076";

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

        rustToolchain = pkgs.rust-bin.selectLatestNightlyWith (toolchain: toolchain.default.override {
          extensions = [ "rust-std" "rust-src" ];
          targets = [ "wasm32-unknown-unknown" "x86_64-pc-windows-gnu" ];
        });

        craneLib = ((crane.mkLib pkgs).overrideToolchain rustToolchain).overrideScope' (_final: _prev: {
          inherit (import nixpkgs-for-wasm-bindgen { inherit system; }) wasm-bindgen-cli;
        });

        src = lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
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
          pname = "rezcraft-native";
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
          pname = "rezcraft-wasm";
          cargoExtraArgs = "--no-default-features --features portable";

          doCheck = false;

          cargoVendorDir = craneLib.vendorMultipleCargoDeps {
            inherit (craneLib.findCargoFiles src) cargoConfigs;
            cargoLockList = [
              ./Cargo.lock
              "${rustToolchain.passthru.availableComponents.rust-src}/lib/rustlib/src/rust/Cargo.lock"
            ];
          };
          nativeBuildInputs = with pkgs; [
            binaryen
            wasm-pack
            wasm-bindgen-cli
          ];

          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";

          buildPhaseCargoCommand = ''
            cargoBuildLog=$(mktemp cargoBuildLogXXXX.json)
            HOME=$(mktemp -d fake-homeXXXX)

            RUSTFLAGS="-C target-feature=+atomics,+bulk-memory,+mutable-globals"
            wasm-pack build --out-dir $out/target/ --target web --features wasm_thread/es_modules -Z build-std=std,panic_abort --message-format json-render-diagnostics > "$cargoBuildLog"
          '';
        };

        nativeCargoArtifacts = craneLib.buildDepsOnly nativeArgs;
        wasmCargoArtifacts = craneLib.buildDepsOnly wasmArgs;

        nativeCrate = craneLib.buildPackage (nativeArgs // {
          cargoArtifacts = nativeCargoArtifacts;

          postInstall = ''
            wrapProgram "$out/bin/rezcraft-native" --set LD_LIBRARY_PATH ${lib.makeLibraryPath runtimeLibs}
            cp -r ./res/ $out/bin/
          '';
        });
        wasmCrate = craneLib.buildPackage (wasmArgs // {
          cargoArtifacts = wasmCargoArtifacts;

          postInstall = ''
            rm -rf $out/lib
            cp ./res/icon.png $out/target/
            cp -a ./res/web/. $out
          '';
        });

        serveWasm = pkgs.writeShellScriptBin "${wasmArgs.pname}" ''
          # ${pkgs.static-web-server}/bin/static-web-server --host 127.0.0.1 --port 8000 --root ${wasmCrate}
          ${pkgs.sfz}/bin/sfz -r
        '';

        nativeCrateClippy = craneLib.cargoClippy (nativeArgs // {
          inherit src;
          cargoArtifacts = nativeCargoArtifacts;

          # cargoClippyExtraArgs = "-- --deny warnings";
        });
      in
      {
        checks = {
          inherit nativeCrate;
          inherit wasmCrate;

          inherit nativeCrateClippy;

          fmt = craneLib.cargoFmt commonArgs;
        };

        packages = {
          rezcraft-native = nativeCrate;
          rezcraft-wasm = wasmCrate;
        };

        apps = {
          rezcraft-native = flake-utils.lib.mkApp {
            name = "rezcraft-native";
            drv = nativeCrate;
          };
          rezcraft-wasm = flake-utils.lib.mkApp {
            name = "rezcraft-wasm";
            drv = serveWasm;
          };
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs;[
            rustToolchain
            runtimeLibs
            wasm-bindgen-cli

            cargo-flamegraph
            cargo-outdated
            gdb

            sfz
          ];

          inherit LD_LIBRARY_PATH;
        };
      });
}
