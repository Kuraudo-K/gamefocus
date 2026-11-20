# GameFocus — à copier dans /etc/nixos/customConfig/apps/gamefocus.nix
# puis activer dans customConfig/default.nix :
#   ./apps/gamefocus.nix
#
# Après push GitHub, mets à jour rev + hash :
#   nix-shell -p nix-prefetch-github --run \
#     'nix-prefetch-github Kuraudo-K gamefocus --rev main'

{ pkgs, lib, ... }:

let
  version = "0.1.0";

  src = pkgs.fetchFromGitHub {
    owner = "Kuraudo-K";
    repo = "gamefocus";
    rev = "f7fa684dc803f5e9e1d7a0724c93541f520f3dd2";
    hash = "sha256-LXt0hWQ49uMI3jt/uRIWrl1dYEaw5jseR+0fIoyPuUE=";
  };

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

  gamefocus = pkgs.rustPlatform.buildRustPackage {
    pname = "gamefocus";
    inherit version src;

    cargoLock.lockFile = "${src}/Cargo.lock";

    nativeBuildInputs = with pkgs; [
      pkg-config
      makeWrapper
      copyDesktopItems
    ];

    buildInputs = runtimeLibs;

    doCheck = false;

    desktopItems = [
      (pkgs.makeDesktopItem {
        name = "gamefocus";
        desktopName = "GameFocus";
        comment = "Bibliothèque de jeux et mode Focus";
        exec = "gamefocus";
        icon = "gamefocus";
        categories = [ "Game" ];
        startupWMClass = "com.gamefocus.GameFocus";
      })
    ];

    postInstall = ''
      mkdir -p $out/share/icons/hicolor/scalable/apps
      cp $src/assets/branding/gamefocus-icon.svg \
        $out/share/icons/hicolor/scalable/apps/gamefocus.svg

      wrapProgram $out/bin/gamefocus \
        --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs} \
        --prefix PATH : ${lib.makeBinPath [ pkgs.icoextract ]}
    '';

    meta = with lib; {
      description = "Bibliothèque de jeux multi-launcher + mode Focus";
      homepage = "https://github.com/Kuraudo-K/gamefocus";
      license = licenses.mit;
      platforms = platforms.linux;
      mainProgram = "gamefocus";
    };
  };
in
{
  environment.systemPackages = [ gamefocus ];
}
