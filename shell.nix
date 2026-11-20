{ pkgs ? import <nixpkgs> { } }:

let
  runtimeLibs = with pkgs; [
    wayland
    libxkbcommon
    libGL
    libglvnd
    libx11
    libxcursor
    libxrandr
    libxi
    libxinerama
    libdecor
  ];
in
pkgs.mkShell {
  packages = with pkgs; [
    gcc
    pkg-config
    rustc
    cargo
    icoextract
  ] ++ runtimeLibs;

  # egui/winit charge Wayland/X11/GL via dlopen — obligatoire sur NixOS / GLF-OS.
  LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath runtimeLibs;
}
