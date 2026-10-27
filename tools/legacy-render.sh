#!/bin/bash
# Render every artifact the legacy bash `delta` generates for a given delta.conf,
# without touching the live system. Used to produce golden files for the Rust port.
#
# Usage: tools/legacy-render.sh <delta.conf> <out-dir>
set -u  # no -e: the legacy helpers rely on grep returning 1

conf="$(realpath "$1")"
out="$2"
repo="$(cd "$(dirname "$0")/.." && pwd)"
mkdir -p "$out"

# Load the legacy functions without running its command dispatch
source <(sed '/^case "\${1:-help}" in/,$d' "$repo/legacy/bash/delta")

# Pin paths so output doesn't depend on where this runs
CONF="$conf"
DELTA_DIR="/home/segger/.local/bin/delta"
LAUNCHER_SCRIPT="$DELTA_DIR/delta-launcher"
XREMAP_CONF="/home/segger/.config/xremap/config.yml"

generate_kwin            > "$out/main.js"
generate_xremap          > "$out/xremap.yml"

generate_launcher_unit   > "$out/delta-launch@.service"

xremap_bin=/usr/bin/xremap
_xremap_service_device_args device_args
cat > "$out/xremap.service" <<UNIT
[Unit]
Description=xremap key remapper (delta modifier)

[Service]
ExecStart=${xremap_bin}${device_args} ${XREMAP_CONF}
Restart=on-failure

[Install]
WantedBy=default.target
UNIT
