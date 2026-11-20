# GameFocus

Application Rust légère pour Linux (NixOS / GLF-OS) : bibliothèque de jeux multi-launcher + suggestions ProtonDB / options de lancement, et **mode Focus** (gèle navigateur / wallpapers, garde Discord, Steam et l’audio).

Launchers supportés : **Steam**, **Heroic**, **Faugus**, **Lutris**.

## NixOS (customConfig GLF)

1. Copie `packaging/gamefocus.nix` → `/etc/nixos/customConfig/apps/gamefocus.nix`
2. Dans `customConfig/default.nix` : `./apps/gamefocus.nix`
3. `sudo nixos-rebuild switch` (ou `glf-update`)

Pour un nouveau commit GitHub, mets à jour `rev` + `hash` :

```bash
nix-shell -p nix-prefetch-github --run \
  'nix-prefetch-github Kuraudo-K gamefocus --rev main'
```

### Dev shell (optionnel)

```bash
nix-shell          # ou : nix develop
cargo build --release
```

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
