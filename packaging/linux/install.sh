#!/bin/sh
# Installs Apix from this folder for the current user (~/.local), or for
# everyone with --system (/usr/local, run with sudo). --uninstall removes it.
set -eu
here="$(cd "$(dirname "$0")" && pwd)"
prefix="$HOME/.local"
remove=false
for arg in "$@"; do
    case "$arg" in
        --system) prefix=/usr/local ;;
        --uninstall) remove=true ;;
        *) echo "usage: $0 [--system] [--uninstall]" >&2; exit 2 ;;
    esac
done

if $remove; then
    rm -f "$prefix/bin/apix-desktop" "$prefix/bin/apix" \
        "$prefix/share/applications/apix.desktop" \
        "$prefix/share/mime/packages/apix.xml"
    find "$prefix/share/icons/hicolor" -name apix.png -delete 2>/dev/null || true
    echo "Apix removed from $prefix"
    exit 0
fi

install -Dm755 "$here/apix-desktop" "$prefix/bin/apix-desktop"
install -Dm755 "$here/apix" "$prefix/bin/apix"
install -Dm644 "$here/apix.desktop" "$prefix/share/applications/apix.desktop"
install -Dm644 "$here/apix.xml" "$prefix/share/mime/packages/apix.xml"
for dir in "$here"/icons/hicolor/*/; do
    size="$(basename "$dir")"
    install -Dm644 "$dir/apps/apix.png" "$prefix/share/icons/hicolor/$size/apps/apix.png"
done
update-desktop-database "$prefix/share/applications" 2>/dev/null || true
update-mime-database "$prefix/share/mime" 2>/dev/null || true
gtk-update-icon-cache -q "$prefix/share/icons/hicolor" 2>/dev/null || true

echo "Apix installed in $prefix"
case ":$PATH:" in
    *":$prefix/bin:"*) ;;
    *) echo "Add $prefix/bin to PATH to run apix and apix-desktop from a terminal." ;;
esac
