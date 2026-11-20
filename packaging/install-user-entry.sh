#!/usr/bin/env bash
# Installe une entrée .desktop utilisateur pointant vers le binaire release local.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/target/release/gamefocus"
APP_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/applications"
ENTRY="$APP_DIR/com.gamefocus.GameFocus.desktop"
ICON_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"

if [[ ! -x "$BIN" ]]; then
  echo "Binaire introuvable. Compile d'abord :" >&2
  echo "  nix develop -c cargo build --release" >&2
  echo "  # ou : nix-shell --run 'cargo build --release'" >&2
  exit 1
fi

mkdir -p "$APP_DIR" "$ICON_DIR"
install -m 0644 "$ROOT/assets/branding/gamefocus-icon.svg" "$ICON_DIR/gamefocus.svg"

# Sur NixOS, wrap via nix-shell pour les libs GL/Wayland.
if [[ -f /etc/NIXOS ]] || [[ -n "${NIX_STORE:-}" ]]; then
  EXEC="nix-shell \"$ROOT/shell.nix\" --run \"$BIN\""
else
  EXEC="\"$BIN\""
fi

cat >"$ENTRY" <<EOF
[Desktop Entry]
Type=Application
Name=GameFocus
Comment=Bibliothèque de jeux et mode Focus
Exec=$EXEC
TryExec=$BIN
Icon=gamefocus
Terminal=false
Categories=Game;
StartupWMClass=com.gamefocus.GameFocus
EOF

echo "Entrée de bureau installée : $ENTRY"
