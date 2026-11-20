{
  description = "GameFocus — bibliothèque de jeux + mode Focus (Linux / NixOS)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;
      runtimeLibs =
        pkgs: with pkgs; [
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
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          libs = runtimeLibs pkgs;
        in
        {
          default = pkgs.rustPlatform.buildRustPackage {
            pname = "gamefocus";
            version = "0.1.0";
            src = ./.;

            cargoLock.lockFile = ./Cargo.lock;

            nativeBuildInputs = with pkgs; [
              pkg-config
              makeWrapper
              copyDesktopItems
            ];

            buildInputs = libs;

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
              cp ${./assets/branding/gamefocus-icon.svg} \
                $out/share/icons/hicolor/scalable/apps/gamefocus.svg

              wrapProgram $out/bin/gamefocus \
                --prefix LD_LIBRARY_PATH : ${pkgs.lib.makeLibraryPath libs}
            '';

            meta = with pkgs.lib; {
              description = "Bibliothèque de jeux multi-launcher + mode Focus pour Linux";
              homepage = "https://github.com/Kuraudo-K/gamefocus";
              license = licenses.mit;
              platforms = platforms.linux;
              mainProgram = "gamefocus";
            };
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.default}/bin/gamefocus";
        };
      });

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          libs = runtimeLibs pkgs;
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              gcc
              pkg-config
              rustc
              cargo
              clippy
              rustfmt
              icoextract
            ] ++ libs;
            LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath libs;
          };
        }
      );
    };
}
