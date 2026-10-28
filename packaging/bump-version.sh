#!/bin/bash
# Set Delta's version everywhere it's written down: the workspace Cargo.toml (all
# crates inherit it), the app's tauri.conf.json and package.json.
#   packaging/bump-version.sh 0.2.0    set it
#   packaging/bump-version.sh --print  show the current one
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
current="$(sed -n 's/^version = "\(.*\)"/\1/p' "$repo/Cargo.toml" | head -1)"

if [[ "${1:-}" == "--print" ]]; then
    echo "$current"
    exit 0
fi
new="${1:?usage: bump-version.sh X.Y.Z | --print}"
[[ "$new" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "not X.Y.Z: $new" >&2; exit 1; }

sed -i "0,/^version = \"$current\"/s//version = \"$new\"/" "$repo/Cargo.toml"
sed -i "s/^  \"version\": \"$current\"/  \"version\": \"$new\"/" "$repo/app/src-tauri/tauri.conf.json" "$repo/app/package.json"
cargo update --workspace --manifest-path "$repo/Cargo.toml" --quiet
grep -q "\"version\": \"$new\"" "$repo/app/src-tauri/tauri.conf.json" "$repo/app/package.json"
echo "$current → $new"
echo "Then: git commit -am 'Delta $new' && git tag v$new && git push origin HEAD v$new"
