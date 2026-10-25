#!/usr/bin/env bash
# Builds the release binary and installs aiditor for this user:
# macOS: ~/Applications/aiditor.app; Linux: ~/.local/bin/aiditor plus a desktop entry and icon.
set -euo pipefail

cd "$(dirname "$0")/.."
res=crates/aiditor/resources
cargo build --release -p aiditor

case "$(uname -s)" in
Darwin)
    version=$(sed -n 's/^version = "\(.*\)"/\1/p' crates/aiditor/Cargo.toml | head -1)
    # Assembled outside target/, which Spotlight indexes, and moved in last, so a failed build keeps the old install.
    tmp=$(mktemp -d)
    app=$tmp/aiditor.app
    mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
    cp target/release/aiditor "$app/Contents/MacOS/aiditor"

    iconset=$tmp/aiditor.iconset
    mkdir "$iconset"
    for size in 16 32 128 256 512; do
        sips -z $size $size "$res/aiditor.png" --out "$iconset/icon_${size}x${size}.png" >/dev/null
        sips -z $((size * 2)) $((size * 2)) "$res/aiditor.png" --out "$iconset/icon_${size}x${size}@2x.png" >/dev/null
    done
    iconutil -c icns "$iconset" -o "$app/Contents/Resources/aiditor.icns"

    cat >"$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key><string>io.github.root-man.aiditor</string>
    <key>CFBundleName</key><string>aiditor</string>
    <key>CFBundleDisplayName</key><string>aiditor</string>
    <key>CFBundleExecutable</key><string>aiditor</string>
    <key>CFBundleIconFile</key><string>aiditor</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>$version</string>
    <key>CFBundleVersion</key><string>$version</string>
    <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
    <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
EOF
    plutil -lint "$app/Contents/Info.plist" >/dev/null
    codesign --force --sign - "$app"
    codesign --verify "$app"
    mkdir -p ~/Applications
    rm -rf ~/Applications/aiditor.app
    mv "$app" ~/Applications/
    echo "Installed ~/Applications/aiditor.app"
    ;;
Linux)
    data=${XDG_DATA_HOME:-$HOME/.local/share}
    bin=$HOME/.local/bin/aiditor
    install -D -m 755 target/release/aiditor "$bin"
    install -D -m 644 "$res/aiditor.svg" "$data/icons/hicolor/scalable/apps/aiditor.svg"
    mkdir -p "$data/applications"
    sed "s|@BIN@|$bin|" "$res/aiditor.desktop" >"$data/applications/aiditor.desktop"
    if command -v desktop-file-validate >/dev/null; then
        desktop-file-validate "$data/applications/aiditor.desktop"
    fi
    update-desktop-database "$data/applications" 2>/dev/null || true
    echo "Installed $bin and $data/applications/aiditor.desktop"
    ;;
*)
    echo "bundle.sh: unsupported platform $(uname -s)" >&2
    exit 1
    ;;
esac
