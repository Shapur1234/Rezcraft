{ pkgs ? import <nixpkgs> { } }:
pkgs.mkShell {
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
}
