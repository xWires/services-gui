{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  shellHook = ''
    export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [ pkgs.wayland pkgs.libxkbcommon ]}:$LD_LIBRARY_PATH"
  '';
}
