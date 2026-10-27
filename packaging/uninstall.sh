#!/bin/bash
# Undo install.sh: stop deltad and put the bash version back. delta.toml is kept.
set -euo pipefail

bin="$HOME/.local/bin"
units="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"
autostart="${XDG_CONFIG_HOME:-$HOME/.config}/autostart/delta-kwin.desktop"

systemctl --user disable --now deltad 2>/dev/null || true
rm -f "$units/deltad.service" "$bin/deltad"
systemctl --user daemon-reload

if [[ -d "$bin/delta.old" ]]; then
    rm -f "$bin/delta"
    [[ -f "$bin/delta.old/delta-kwin.desktop" ]] && mv "$bin/delta.old/delta-kwin.desktop" "$autostart"
    mv "$bin/delta.old" "$bin/delta"
    # Regenerate the bash version's files (launcher unit, KWin script, xremap)
    "$bin/delta/delta" apply
    "$bin/delta/delta" render-launcher
fi
echo "Restored the bash version. Re-add ~/.local/bin/delta to PATH if you removed it."
