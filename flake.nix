{
  description = "Rezcraft";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.rust-analyzer-src.follows = "";
    };

    flake-utils.url = "github:numtide/flake-utils";

    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };

  };

  outputs = { self, nixpkgs, crane, fenix, flake-utils, advisory-db, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ ];
        };
        inherit (pkgs) lib;

        runtimeLibs = with pkgs; [
          vulkan-headers
          vulkan-loader
          vulkan-tools
          vulkan-validation-layers

          wayland
          wayland-protocols

          xorg.libX11
          xorg.libXcursor
          xorg.libXi
          xorg.libXrandr
        ];

        src = craneLib.cleanCargoSource (craneLib.path ./.);

        craneLib = crane.lib.${system};
        craneLibLLvmTools = craneLib.overrideToolchain
          (fenix.packages.${system}.complete.withComponents [
            "cargo"
            "llvm-tools"
            "rustc"
          ]);

        commonArgs = {
          inherit src;
          strictDeps = true;

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

          my-crate-audit = craneLib.cargoAudit {
            inherit src advisory-db;
          };
        };

        packages = {
          default = rezcraft;
          my-crate-llvm-coverage = craneLibLLvmTools.cargoLlvmCov (commonArgs // {
            inherit cargoArtifacts;
          });
        };

        apps.default = flake-utils.lib.mkApp {
          drv = rezcraft;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          packages = with pkgs;[
            cargo-flamegraph
            gdb
          ];

          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
        };
      });
}
