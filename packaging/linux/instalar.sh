#!/usr/bin/env bash
# Instala TransFEL para el usuario actual. No necesita root.
set -euo pipefail

AQUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DESTINO="$HOME/.local/bin"
ESCRITORIO="$HOME/.local/share/applications"

echo "TransFEL - instalacion"
echo

# ── Dependencias ──
faltan=()
command -v adb    >/dev/null 2>&1 || faltan+=("adb")
command -v ffmpeg >/dev/null 2>&1 || faltan+=("ffmpeg")

if [ ${#faltan[@]} -gt 0 ]; then
    echo "Faltan dependencias: ${faltan[*]}"
    echo
    if   command -v apt    >/dev/null 2>&1; then echo "  sudo apt install ${faltan[*]}"
    elif command -v dnf    >/dev/null 2>&1; then echo "  sudo dnf install ${faltan[*]/adb/android-tools}"
    elif command -v pacman >/dev/null 2>&1; then echo "  sudo pacman -S ${faltan[*]/adb/android-tools}"
    elif command -v zypper >/dev/null 2>&1; then echo "  sudo zypper install ${faltan[*]/adb/android-tools}"
    else echo "  Instalalas con el gestor de paquetes de tu distribucion."
    fi
    echo
    read -rp "Continuar de todos modos? [s/N] " respuesta
    [[ "$respuesta" =~ ^[sS]$ ]] || exit 1
fi

# ── Copiar ──
mkdir -p "$DESTINO" "$ESCRITORIO"
install -m 755 "$AQUI/transfel" "$DESTINO/transfel"
install -m 644 "$AQUI/transfel.desktop" "$ESCRITORIO/transfel.desktop"

command -v update-desktop-database >/dev/null 2>&1 \
    && update-desktop-database "$ESCRITORIO" 2>/dev/null || true

echo "Instalado en $DESTINO/transfel"

case ":$PATH:" in
    *":$DESTINO:"*) ;;
    *) echo
       echo "Aviso: $DESTINO no esta en tu PATH."
       echo "Anade esto a tu ~/.bashrc o ~/.zshrc:"
       echo "    export PATH=\"\$HOME/.local/bin:\$PATH\""
       ;;
esac

echo
echo "Para usar ADB sin sudo, tu usuario debe estar en el grupo adecuado:"
echo "    sudo usermod -aG plugdev \$USER    # Debian/Ubuntu"
echo "Y despues cerrar sesion y volver a entrar."
echo
echo "Ejecuta:  transfel"
