#!/usr/bin/env bash
set -euo pipefail
rm -f "$HOME/.local/bin/transfel"
rm -f "$HOME/.local/share/applications/transfel.desktop"
rm -rf "${XDG_CONFIG_HOME:-$HOME/.config}/transfel"
echo "TransFEL desinstalado."
