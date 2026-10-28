#!/bin/bash
# Put the programs the released app installs where Tauri expects them: deltad and delta
# as externalBin (app/src-tauri/binaries/<name>-<target triple>), xremap as the
# resource app/src-tauri/binaries/xremap.gz.
# Run by `tauri build --config src-tauri/tauri.release.conf.json`.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
out="$repo/app/src-tauri/binaries"
triple="$(rustc -vV | sed -n 's/^host: //p')"

# xremap's KDE build from its GitHub releases (MIT). Same version Freya ran before the
# switch; bump both lines together.
xremap_version="v0.15.8"
xremap_sha256="cb68521995a72e4567e4bb6e5afe9f2366df722ec75d94aa4be808d18d8f6dd1"

case "$triple" in
    x86_64-unknown-linux-gnu) xremap_arch=x86_64 ;;
    aarch64-unknown-linux-gnu) xremap_arch=aarch64 ;;
    *) echo "no xremap build for $triple" >&2; exit 1 ;;
esac

echo "==> Building deltad and delta"
cargo build --release --manifest-path "$repo/Cargo.toml" -p deltad -p delta-cli
mkdir -p "$out"
install -m755 "$repo/target/release/deltad" "$out/deltad-$triple"
install -m755 "$repo/target/release/delta" "$out/delta-$triple"

zip="$repo/target/xremap/xremap-$xremap_version-$xremap_arch-kde.zip"
if [[ ! -f "$zip" ]]; then
    echo "==> Downloading xremap $xremap_version"
    mkdir -p "$(dirname "$zip")"
    curl -fsSL -o "$zip.part" \
        "https://github.com/xremap/xremap/releases/download/$xremap_version/xremap-linux-$xremap_arch-kde.zip"
    mv "$zip.part" "$zip"
fi
echo "$xremap_sha256  $zip" | sha256sum -c --quiet
# Gzipped, as a resource: linuxdeploy patches every ELF it finds in the AppImage, and
# patching xremap's static binary makes it segfault
rm -f "$out/xremap-$triple"
unzip -p "$zip" xremap | gzip -9n > "$out/xremap.gz"
echo "==> Staged in $out"
