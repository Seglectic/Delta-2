# Intentional differences from the legacy bash output

`expected/` started as the output of `tools/legacy-render.sh` (the bash generator).
These differences are on purpose:

**Both fixtures**
- `main.js` header comment points at `delta.toml` (the golden test skips the header anyway).
- `main.js` app matching is data-driven: an `appProfiles` table generated from `[[apps]]`
  replaces the hard-coded `if (profile === "discord")` checks. Window titles are no longer
  matched, so a browser tab titled "Discord" isn't mistaken for Discord. The legacy
  "tile Discord into the left third on focus" became `focus_tile` on the discord app.

**edge only**
- `[overrides]` app class is everything before the *last* dot (legacy split on the first:
  `org.kde.konsole.Q` became app `org`).
- `[app_bindings]` for one app are grouped into a single xremap block (legacy emitted a
  duplicate block when an app's entries weren't adjacent).

xremap config, xremap service and launcher unit are still byte-identical to legacy.
