# GameFocus

Application Rust légère pour Linux (NixOS / GLF-OS) : bibliothèque de jeux multi-launcher + suggestions ProtonDB / options de lancement, et **mode Focus** (gèle navigateur / wallpapers, garde Discord, Steam et l’audio).

Launchers supportés : **Steam**, **Heroic**, **Faugus**, **Lutris**.

## NixOS (customConfig GLF)

Comme Lunii / Hytale : un fichier `.nix` qui tire le dépôt GitHub.

1. Pousse ce repo sur GitHub (`Kuraudo-K/gamefocus`)
2. Copie `packaging/gamefocus.nix` → `/etc/nixos/customConfig/apps/gamefocus.nix`
3. Mets le `rev` + `hash` :

```bash
nix-shell -p nix-prefetch-github --run \
  'nix-prefetch-github Kuraudo-K gamefocus --rev main'
```

4. Dans `customConfig/default.nix` :

```nix
./apps/gamefocus.nix
```

5. `sudo nixos-rebuild switch` (ou `glf-update`)

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
