#!/usr/bin/env bash
# Entrée .desktop pour un build local (cargo). Sur NixOS/GLF avec le paquet
# système, ne pas utiliser ce script : l’icône vient déjà de nixos-rebuild.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/gamefocus"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ENTRY="$APP_DIR/com.gamefocus.GameFocus.desktop"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
LOCAL_BIN="${XDG_BIN_HOME:-$HOME/.local/bin}"

if [[ ! -x "$BIN" ]]; then
  echo "Binaire introuvable. Compile d'abord :" >&2
  echo "  nix-shell --run 'cargo build --release'" >&2
  exit 1
fi

# Si le paquet Nix est installé, l’entrée système (Exec=gamefocus) suffit.
if command -v gamefocus >/dev/null 2>&1; then
  SYS="$(command -v gamefocus)"
  case "$SYS" in
    /run/current-system/*|/nix/store/*)
      echo "GameFocus système déjà présent ($SYS)."
      echo "Pas d’entrée ~/.local : retire les anciens fichiers s’ils existent :"
      echo "  rm -f \"$LOCAL_BIN/gamefocus\" \"$APP_DIR/gamefocus.desktop\" \"$ENTRY\""
      exit 0
      ;;
  esac
fi

mkdir -p "$APP_DIR" "$ICON_DIR" "$LOCAL_BIN"
install -m 0644 "$ROOT/assets/branding/gamefocus-icon.svg" "$ICON_DIR/gamefocus.svg"
ln -sfn "$BIN" "$LOCAL_BIN/gamefocus"

# Exec=gamefocus (PATH), pas un chemin /nix/store figé qui casse au GC.
cat >"$ENTRY" <<EOF
[Desktop Entry]
Type=Application
Name=GameFocus
Comment=Bibliothèque de jeux et mode Focus
Exec=gamefocus
TryExec=$LOCAL_BIN/gamefocus
Icon=gamefocus
Terminal=false
Categories=Game;
StartupWMClass=com.gamefocus.GameFocus
EOF

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$APP_DIR" 2>/dev/null || true
fi

echo "Entrée de bureau installée : $ENTRY"
echo "Symlink : $LOCAL_BIN/gamefocus → $BIN"
