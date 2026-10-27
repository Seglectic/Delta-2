Expected output here is the legacy bash output with two intentional fixes:
- [overrides] app class is everything before the LAST dot (legacy split on the first: org.kde.konsole.Q became app "org")
- [app_bindings] for one app are grouped into a single xremap block (legacy emitted a duplicate block when entries were not adjacent)
