{
  description = "Voxel engine written in rust using wgpu, supporting both native and wasm ";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";

    crane = {
      url = "github:ipetkov/crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, crane, flake-utils, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
        };

        craneLib = crane.lib.${system};
        rezcraft = craneLib.buildPackage {
          src = craneLib.cleanCargoSource (craneLib.path ./.);

          buildInputs = [ ];
        };
      in
      {
        checks = {
          my-crate = rezcraft;
        };

        packages.default = rezcraft;

        apps.default = flake-utils.lib.mkApp {
          drv = rezcraft;
        };

        devShells.default = craneLib.devShell {
          shellHook = ''
            export LD_LIBRARY_PATH="$LD_LIBRARY_PATH:${
              with pkgs;
              lib.makeLibraryPath [
                libxkbcommon
                libGL

                wayland

                xorg.libXcursor
                xorg.libXrandr
                xorg.libXi
                xorg.libX11
              ]
            }"
          '';
        };
      });
}
