# 𝚫2

Caps Lock becomes a layer key. Hold it and tap a key to tile windows, focus apps and run actions.

This branch (`delta-next`) is turning Delta into an all-in-one keyboard hotkey and hardware engine for KDE Plasma: a background service (`deltad`), a CLI (`delta`), and a GUI that draws your real keyboard from its QMK firmware.

- **Current working version (bash + xremap):** [`legacy/bash/`](legacy/bash/); full docs in [`legacy/bash/agents.md`](legacy/bash/agents.md)
- **Original Windows version (AutoHotkey):** [`windows/`](windows/)

## Keyboard layouts

Delta draws your keyboard from its VIA definition, fetched from usevia.app by USB ID the
first time a board is seen and cached in `~/.cache/delta/layouts/`. To use your own,
put the VIA JSON in `~/.config/delta/layouts/`; those win. `delta keyboard list` shows
what was matched.
