# Keyboard layouts

VIA keyboard definitions: physical key positions (KLE format) plus matrix size and the
board's custom keycodes. Firmware only knows keycodes per matrix position, so these
are what let Delta draw the real keyboard.

| File | Board | USB ID | Source |
|---|---|---|---|
| `keychron/q5_ansi_encoder.json` | Keychron Q5 ANSI Knob | 3434:0151 | [the-via/keyboards](https://github.com/the-via/keyboards) `v3/keychron/q5/ansi_encoder.json` (GPL-3.0) |

Files here are compiled into `delta-via`. For another board, drop its VIA JSON into
`~/.config/delta/layouts/`; `delta keyboard list` shows whether it was matched.
