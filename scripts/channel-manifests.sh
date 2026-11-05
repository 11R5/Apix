#!/usr/bin/env bash
# Fills in the Homebrew cask and the winget manifests for a release from the
# files in a folder (the release's .dmg and setup .exe):
#   scripts/channel-manifests.sh <version> <folder with the release files> <out folder>
# Writes <out>/homebrew/Casks/apix.rb and <out>/winget/<version>/*.yaml, ready
# to copy into the tap repository and a microsoft/winget-pkgs pull request.
set -euo pipefail
version="$1"
files="$(cd "$2" && pwd)"
mkdir -p "$3"
out="$(cd "$3" && pwd)"
cd "$(dirname "$0")/.."

sum() { sha256sum "$1" | cut -d' ' -f1; }
dmg="$files/Apix-$version-macos-universal.dmg"
exe="$files/Apix-$version-windows-x64-setup.exe"

mkdir -p "$out/homebrew/Casks" "$out/winget/$version"
if [ -f "$dmg" ]; then
    sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$(sum "$dmg")/g" \
        packaging/homebrew/apix.rb > "$out/homebrew/Casks/apix.rb"
fi
if [ -f "$exe" ]; then
    exe_sum="$(sum "$exe" | tr '[:lower:]' '[:upper:]')"
    for manifest in packaging/winget/*.yaml; do
        sed -e "s/@VERSION@/$version/g" -e "s/@SHA256@/$exe_sum/g" \
            "$manifest" > "$out/winget/$version/$(basename "$manifest")"
    done
fi
find "$out" -type f
