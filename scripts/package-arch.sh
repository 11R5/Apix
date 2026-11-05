#!/usr/bin/env bash
# Makes the Arch package (dist/apix-<version>-1-x86_64.pkg.tar.zst) from
# dist/PKGBUILD and the tarball next to it, without downloading anything.
# Runs in Arch Linux (or the archlinux container) after scripts/package-linux.sh;
# makepkg refuses root, so as root it builds as a throwaway user.
set -euo pipefail
cd "$(dirname "$0")/.."
dist="$(pwd)/dist"
work="$(mktemp -d)"
cp "$dist/PKGBUILD" "$dist"/apix-*-linux-x64.tar.gz "$work/"
# The tarball is already here: point the source at it.
sed -i 's|^source=("https://.*/\(apix-${pkgver}-linux-x64.tar.gz\)")|source=("\1")|' "$work/PKGBUILD"
if [ "$(id -u)" = 0 ]; then
    id builder >/dev/null 2>&1 || useradd -m builder
    chown -R builder "$work"
    su builder -c "cd '$work' && makepkg --nodeps --noconfirm"
else
    (cd "$work" && makepkg --nodeps --noconfirm)
fi
cp "$work"/apix-*.pkg.tar.zst "$dist/"
rm -rf "$work"
ls -lh "$dist"/apix-*.pkg.tar.zst
