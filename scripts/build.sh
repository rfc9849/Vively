#!/bin/bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

APP_NAME="Vively"
BUNDLE_DIR="$ROOT_DIR/target/release/$APP_NAME.app"

build_mac() {
    if [ "$(uname -s)" != Darwin ]; then
        printf 'mac requires macOS\n' >&2
        return 1
    fi
    local bundle_version
    bundle_version=$(awk -F '"' '/^version = "/ { print $2; exit }' crates/vively/Cargo.toml)
    bundle_version=${bundle_version%%[-+]*}
    if [ -z "$bundle_version" ]; then
        printf 'Cannot read Vively version from Cargo.toml\n' >&2
        return 1
    fi

    printf '\033[1;34m▸\033[0m Building %s (release)...\n' "$APP_NAME"
    cargo build --release --locked -p vively

    rm -rf "$BUNDLE_DIR"
    mkdir -p "$BUNDLE_DIR/Contents/MacOS" "$BUNDLE_DIR/Contents/Resources"

    cp "$ROOT_DIR/target/release/vively" "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"
    chmod +x "$BUNDLE_DIR/Contents/MacOS/$APP_NAME"

    if [ -f "$ROOT_DIR/assets/AppIcon.icns" ]; then
        cp "$ROOT_DIR/assets/AppIcon.icns" "$BUNDLE_DIR/Contents/Resources/AppIcon.icns"
    fi

    printf 'APPL????' > "$BUNDLE_DIR/Contents/PkgInfo"

    cat > "$BUNDLE_DIR/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>ru</string>
    <key>CFBundleDisplayName</key>
    <string>Vively</string>
    <key>CFBundleExecutable</key>
    <string>Vively</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundleIconName</key>
    <string>AppIcon</string>
    <key>CFBundleIdentifier</key>
    <string>eu.r3pr3ss10n.vively</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Vively</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>14.0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSPrincipalClass</key>
    <string>NSApplication</string>
</dict>
</plist>
PLIST

    /usr/libexec/PlistBuddy -c "Add :CFBundleShortVersionString string $bundle_version" "$BUNDLE_DIR/Contents/Info.plist"
    codesign --force --deep --sign - "$BUNDLE_DIR"
    printf '\033[1;34m▸\033[0m Built %s\n' "$BUNDLE_DIR"
}

install_mac() {
    build_mac
    local installed_app="/Applications/$APP_NAME.app"
    printf '\033[1;34m▸\033[0m Replacing %s...\n' "$installed_app"
    osascript -e "tell application \"$APP_NAME\" to quit" >/dev/null 2>&1 || true
    rm -rf "$installed_app"
    cp -R "$BUNDLE_DIR" "/Applications/"
    printf '\033[1;34m▸\033[0m Installed %s\n' "$installed_app"
}

build_linux() {
    if [ "$(uname -s)" != Linux ]; then
        printf 'linux requires a Linux build environment\n' >&2
        return 1
    fi
    cargo build --release --locked -p vively
}

case "$(uname -s)" in
    Linux) default_target=linux ;;
    *) default_target=mac ;;
esac

case "${1:-$default_target}" in
    mac) build_mac ;;
    install) install_mac ;;
    linux) build_linux ;;
    *) printf 'usage: %s [mac|install|linux]\n' "$0" >&2; exit 1 ;;
esac
