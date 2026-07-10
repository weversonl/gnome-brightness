#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

echo "==> Compilando (release)"
cargo build --release

BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
LOCALE_DIR="$HOME/.local/share/locale"

mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICON_DIR"

echo "==> Instalando binário"
install -m 755 target/release/gnome-brightness "$BIN_DIR/gnome-brightness"

echo "==> Instalando ícone"
install -m 644 data/icons/com.verso.GnomeBrightness.svg "$ICON_DIR/com.verso.GnomeBrightness.svg"

echo "==> Instalando .desktop"
install -m 644 data/com.verso.GnomeBrightness.desktop.in "$APPS_DIR/com.verso.GnomeBrightness.desktop"

echo "==> Compilando traduções"
for po in po/*.po; do
    lang="$(basename "$po" .po)"
    mo_dir="$LOCALE_DIR/$lang/LC_MESSAGES"
    mkdir -p "$mo_dir"
    msgfmt "$po" -o "$mo_dir/gnome-brightness.mo"
done

update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true

echo "==> Concluído. O app aparece no menu do GNOME como 'Brilho dos Monitores'."

read -rp "Iniciar automaticamente com a sessão? [s/N] " answer
if [[ "$answer" =~ ^[sSyY]$ ]]; then
    mkdir -p "$HOME/.config/autostart"
    install -m 644 data/com.verso.GnomeBrightness.desktop.in \
        "$HOME/.config/autostart/com.verso.GnomeBrightness.desktop"
    echo "==> Autostart configurado."
fi
