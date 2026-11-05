#!/usr/bin/env bash
# Builds dist/Apix-<version>-macos-universal.dmg: Apix.app for Apple silicon
# and Intel Macs in one, with the `apix` command line inside
# (Apix.app/Contents/MacOS/apix). The app is signed ad hoc, not notarized.
# Usage (from the repository root, on a Mac): scripts/package-macos.sh [version]
set -euo pipefail
cd "$(dirname "$0")/.."

version="${1:-$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)}"
targets=(aarch64-apple-darwin x86_64-apple-darwin)

for target in "${targets[@]}"; do
    rustup target add "$target" >/dev/null
    MACOSX_DEPLOYMENT_TARGET=11.0 cargo build --release -p apix-desktop -p apix-cli --target "$target"
done

dist=dist
app="$dist/Apix.app"
rm -rf "$app" "$dist/dmg"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
for bin in apix-desktop apix; do
    lipo -create -output "$app/Contents/MacOS/$bin" \
        "target/aarch64-apple-darwin/release/$bin" \
        "target/x86_64-apple-darwin/release/$bin"
done
sed "s/@VERSION@/$version/g" packaging/macos/Info.plist > "$app/Contents/Info.plist"
cp packaging/macos/Apix.icns "$app/Contents/Resources/Apix.icns"
cp LICENSE "$app/Contents/Resources/LICENSE"
# Apple silicon refuses unsigned code; an ad-hoc signature is enough to run
# after the first "Open" (or `xattr -dr com.apple.quarantine`).
codesign --force --deep --sign - "$app"

mkdir -p "$dist/dmg"
cp -R "$app" "$dist/dmg/"
ln -s /Applications "$dist/dmg/Applications"
cp LICENSE "$dist/dmg/LICENSE.txt"
dmg="$dist/Apix-$version-macos-universal.dmg"
rm -f "$dmg"
hdiutil create -volname "Apix $version" -srcfolder "$dist/dmg" -ov -format UDZO "$dmg"
rm -rf "$dist/dmg"
# The command line alone, for the GitHub Action.
tar -C "$app/Contents/MacOS" -czf "$dist/apix-cli-$version-macos-universal.tar.gz" apix
ls -lh "$dmg" "$dist/apix-cli-$version-macos-universal.tar.gz"
