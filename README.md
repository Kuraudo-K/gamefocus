# GameFocus

Application Rust légère pour Linux (NixOS / GLF-OS) : bibliothèque de jeux multi-launcher + suggestions ProtonDB / options de lancement, et **mode Focus** (gèle navigateur / wallpapers, garde Discord, Steam et l’audio).

Launchers supportés : **Steam**, **Heroic**, **Faugus**, **Lutris**.

## Nix / NixOS

### Dev shell

```bash
nix develop
cargo run
# ou
cargo build --release && ./target/release/gamefocus
```

Sans flake :

```bash
nix-shell
cargo build --release
```

### Installer / lancer le paquet

```bash
# build
nix build

# lancer
nix run

# ou installer dans le profil
nix profile install .#default
```

### Ajouter à ta config NixOS (exemple)

```nix
# flake.nix
{
  inputs.gamefocus.url = "github:Kuraudo-K/gamefocus";
  # ...
  outputs = { nixpkgs, gamefocus, ... }: {
    nixosConfigurations.tonpc = nixpkgs.lib.nixosSystem {
      modules = [
        ({ pkgs, ... }: {
          environment.systemPackages = [
            gamefocus.packages.${pkgs.system}.default
          ];
        })
      ];
    };
  };
}
```

Le binaire est wrappé avec `LD_LIBRARY_PATH` (Wayland / X11 / GL) pour egui.

## Build classique (hors Nix)

```bash
cargo build --release
./target/release/gamefocus
```

## Tests

```bash
nix develop -c cargo test --all-targets
# ou
cargo test --all-targets
```

## CLI

```text
gamefocus                 UI
gamefocus --scan          Scan bibliothèques
gamefocus --focus-on      Active le Focus
gamefocus --focus-off     Quitte le Focus
gamefocus --demo          Cycle Focus dry-run
```

Variables utiles : `GAMEFOCUS_DRY_RUN=1`, `GAMEFOCUS_DEMO=1`.

## Architecture

Hexagonale (domaine / ports / application / adapters) — le domaine ne dépend pas d’`eframe`, `ureq` ni `sysinfo`.

## Licence

MIT
