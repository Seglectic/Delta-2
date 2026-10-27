#!/bin/bash
# Install Delta (deltad + delta) for the current user and retire the bash version.
# Undo with packaging/uninstall.sh.
set -euo pipefail

repo="$(cd "$(dirname "$0")/.." && pwd)"
bin="$HOME/.local/bin"
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
config="${XDG_CONFIG_HOME:-$HOME/.config}/delta/delta.toml"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/delta-kwin.desktop"

echo "==> Building"
cargo build --release --manifest-path "$repo/Cargo.toml" -p deltad -p delta-cli

# The bash version lived in a directory called ~/.local/bin/delta, which the new
# binary needs as its name. Keep it as delta.old so uninstall can restore it.
if [[ -d "$bin/delta" ]]; then
    echo "==> Moving the bash version to $bin/delta.old"
    mv "$bin/delta" "$bin/delta.old"
fi

echo "==> Installing deltad and delta to $bin"
install -Dm755 "$repo/target/release/deltad" "$bin/deltad"
install -Dm755 "$repo/target/release/delta" "$bin/delta"

if [[ ! -f "$config" && -f "$bin/delta.old/delta.conf" ]]; then
    echo "==> Importing $bin/delta.old/delta.conf"
    "$bin/delta" --config "$config" import --from "$bin/delta.old/delta.conf"
fi

# deltad loads the KWin script itself once KWin is up
if [[ -f "$autostart" ]]; then
    echo "==> Retiring the KWin autostart entry"
    mv "$autostart" "$bin/delta.old/delta-kwin.desktop"
fi

echo "==> Starting deltad"
install -Dm644 "$repo/packaging/deltad.service" "$units/deltad.service"
systemctl --user daemon-reload
systemctl --user enable --now deltad
sleep 2
"$bin/delta" status
