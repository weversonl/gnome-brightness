#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

locale_tag="${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}"
if [[ "$locale_tag" == pt_BR* ]]; then
    MSG_BUILD="==> Compilando (release)"
    MSG_BIN="==> Instalando binário"
    MSG_ICON="==> Instalando ícone"
    MSG_DESKTOP="==> Instalando .desktop"
    MSG_I18N="==> Compilando traduções"
    MSG_DONE="==> Concluído. O app aparece no menu do GNOME como 'GnomeBrightness'."
    MSG_AUTOSTART_PROMPT="Iniciar automaticamente com a sessão? [s/N] "
    MSG_AUTOSTART_DONE="==> Autostart configurado."
else
    MSG_BUILD="==> Building (release)"
    MSG_BIN="==> Installing binary"
    MSG_ICON="==> Installing icon"
    MSG_DESKTOP="==> Installing .desktop entry"
    MSG_I18N="==> Compiling translations"
    MSG_DONE="==> Done. The app shows up in the GNOME menu as 'GnomeBrightness'."
    MSG_AUTOSTART_PROMPT="Start automatically on login? [y/N] "
    MSG_AUTOSTART_DONE="==> Autostart configured."
fi

echo "$MSG_BUILD"
cargo build --release

BIN_DIR="$HOME/.local/bin"
APPS_DIR="$HOME/.local/share/applications"
ICON_DIR="$HOME/.local/share/icons/hicolor/scalable/apps"
LOCALE_DIR="$HOME/.local/share/locale"

mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICON_DIR"

echo "$MSG_BIN"
install -m 755 target/release/gnome-brightness "$BIN_DIR/gnome-brightness"

echo "$MSG_ICON"
install -m 644 data/icons/io.github.weversonl.GnomeBrightness.svg "$ICON_DIR/io.github.weversonl.GnomeBrightness.svg"

echo "$MSG_DESKTOP"
sed "s|@BINDIR@|$BIN_DIR|g" data/io.github.weversonl.GnomeBrightness.desktop.in > "$APPS_DIR/io.github.weversonl.GnomeBrightness.desktop"
chmod 644 "$APPS_DIR/io.github.weversonl.GnomeBrightness.desktop"

echo "$MSG_I18N"
for po in po/*.po; do
    lang="$(basename "$po" .po)"
    mo_dir="$LOCALE_DIR/$lang/LC_MESSAGES"
    mkdir -p "$mo_dir"
    msgfmt "$po" -o "$mo_dir/gnome-brightness.mo"
done

update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
gtk-update-icon-cache "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true

echo "$MSG_DONE"

read -rp "$MSG_AUTOSTART_PROMPT" answer
if [[ "$answer" =~ ^[sSyY]$ ]]; then
    mkdir -p "$HOME/.config/autostart"
    sed "s|@BINDIR@|$BIN_DIR|g" data/io.github.weversonl.GnomeBrightness.desktop.in \
        > "$HOME/.config/autostart/io.github.weversonl.GnomeBrightness.desktop"
    chmod 644 "$HOME/.config/autostart/io.github.weversonl.GnomeBrightness.desktop"
    echo "$MSG_AUTOSTART_DONE"
fi
