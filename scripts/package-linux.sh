#!/usr/bin/env bash
# Builds the Linux release files into dist/:
#   apix_<version>_amd64.deb            Debian, Ubuntu, Mint, Pop!_OS, …
#   apix-<version>-linux-x64.tar.gz     any distribution: unpack, ./install.sh
#   PKGBUILD                            Arch (makepkg, or the AUR) from that tarball
# The Arch package itself (.pkg.tar.zst) is made from the PKGBUILD by
# scripts/package-arch.sh in an Arch system or container.
# Usage (from the repository root): scripts/package-linux.sh [version] [--no-build]
set -euo pipefail
cd "$(dirname "$0")/.."

version="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
build=true
for arg in "$@"; do
    case "$arg" in
        --no-build) build=false ;;
        *) version="$arg" ;;
    esac
done
if $build; then
    cargo build --release -p apix-desktop -p apix-cli
fi
bin=target/release
dist=dist
mkdir -p "$dist"

# --- portable tarball -------------------------------------------------------
name="apix-$version-linux-x64"
stage="$dist/$name"
rm -rf "$stage"
mkdir -p "$stage"
install -m755 "$bin/apix-desktop" "$bin/apix" packaging/linux/install.sh "$stage/"
install -m644 packaging/linux/apix.desktop packaging/linux/apix.xml LICENSE "$stage/"
cp -R packaging/linux/icons "$stage/icons"
tar -C "$dist" -czf "$dist/$name.tar.gz" "$name"
rm -rf "$stage"

# --- .deb --------------------------------------------------------------------
root="$dist/deb"
rm -rf "$root"
install -Dm755 "$bin/apix-desktop" "$root/usr/bin/apix-desktop"
install -Dm755 "$bin/apix" "$root/usr/bin/apix"
install -Dm644 packaging/linux/apix.desktop "$root/usr/share/applications/apix.desktop"
install -Dm644 packaging/linux/apix.xml "$root/usr/share/mime/packages/apix.xml"
install -Dm644 LICENSE "$root/usr/share/doc/apix/copyright"
for dir in packaging/linux/icons/hicolor/*/; do
    size="$(basename "$dir")"
    install -Dm644 "$dir/apps/apix.png" "$root/usr/share/icons/hicolor/$size/apps/apix.png"
done
# The libraries the programs link against, as Debian package names. Vulkan
# is loaded at run time, so it is named by hand.
mkdir -p "$dist/shlibs/debian"
echo "Source: apix" > "$dist/shlibs/debian/control"
depends="$(cd "$dist/shlibs" && dpkg-shlibdeps -O "$OLDPWD/$root/usr/bin/apix-desktop" "$OLDPWD/$root/usr/bin/apix" 2>/dev/null \
    | sed -n 's/^shlibs:Depends=//p')"
rm -rf "$dist/shlibs"
mkdir -p "$root/DEBIAN"
cat > "$root/DEBIAN/control" <<EOF
Package: apix
Version: $version
Architecture: amd64
Maintainer: Ahmet Demir <46265066+AhmetVDemir@users.noreply.github.com>
Depends: ${depends:+$depends, }libvulkan1
Recommends: mesa-vulkan-drivers
Section: devel
Priority: optional
Homepage: https://github.com/11R5/Apix
Description: Fast, native API client
 Apix sends HTTP, GraphQL, gRPC, WebSocket, Socket.IO and MQTT requests
 kept as plain files next to your code. It comes with the apix command line
 for terminals and CI.
EOF
dpkg-deb --build --root-owner-group "$root" "$dist/apix_${version}_amd64.deb"
rm -rf "$root"

# --- PKGBUILD for Arch -----------------------------------------------------------
sum="$(sha256sum "$dist/$name.tar.gz" | cut -d' ' -f1)"
sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$sum/" packaging/arch/PKGBUILD > "$dist/PKGBUILD"

ls -lh "$dist"
