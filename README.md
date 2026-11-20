# GameFocus

Bibliothèque de jeux multi-launcher + mode Focus pour Linux (NixOS / GLF-OS).

Launchers : **Steam**, **Heroic**, **Faugus**, **Lutris**.

## Install — GLF-OS / customConfig (npins)

1. **Pin** — dans `customConfig/system/verify-sources.nix`, ajoute dans `expectedPins` :

```nix
{ name = "gamefocus"; addArgs = "github Kuraudo-K gamefocus --branch main"; }
```

2. **Source** — dans `customConfig/system/sources.nix` :

```nix
gamefocus = sources.gamefocus or null;
```

3. **Module** — copie `packaging/gamefocus-npins.nix` vers :

```text
/etc/nixos/customConfig/apps/gamefocus.nix
```

4. **Import** — dans `customConfig/default.nix` :

```nix
./apps/gamefocus.nix
```

5. **Télécharge le pin + rebuild** :

```bash
glf-sha
# ou : cd /etc/nixos/customConfig && npins add github Kuraudo-K gamefocus --branch main
sudo nixos-rebuild switch   # ou glf-update
```

Mise à jour plus tard : `glf-sha` puis rebuild.

## Install — NixOS sans npins

1. Copie `packaging/gamefocus.nix` → ton module (ex. `apps/gamefocus.nix`)
2. Importe-le dans ta config
3. Rebuild

Après un nouveau commit GitHub, mets à jour `rev` + `hash` dans ce fichier :

```bash
nix-shell -p nix-prefetch-github --run \
  'nix-prefetch-github Kuraudo-K gamefocus --rev main'
```

## Dev / build local

```bash
nix-shell          # ou : nix develop
cargo build --release
./target/release/gamefocus
```

Icône menu (build local seulement) :

```bash
./packaging/install-user-entry.sh
```

Si Plasma dit qu’il ne trouve pas `~/.local/bin/gamefocus` alors que Nix
installe déjà l’appli : retire l’ancien lanceur local, puis relance le menu.

```bash
rm -f ~/.local/bin/gamefocus \
      ~/.local/share/applications/gamefocus.desktop \
      ~/.local/share/applications/com.gamefocus.GameFocus.desktop
update-desktop-database ~/.local/share/applications 2>/dev/null || true
```

## Tests

```bash
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

Variables : `GAMEFOCUS_DRY_RUN=1`, `GAMEFOCUS_DEMO=1`.

## Licence

MIT
