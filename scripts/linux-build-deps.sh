#!/usr/bin/env bash
set -euo pipefail

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential clang cmake pkg-config libasound2-dev libdbus-1-dev libfontconfig1-dev libgtk-3-dev libssl-dev libudev-dev libvulkan-dev libwayland-dev libwebkit2gtk-4.1-dev libx11-xcb-dev libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-x11-dev
