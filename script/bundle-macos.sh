#!/usr/bin/env bash
# Wraps the `vigia-app` binary (and the `vigia` CLI) in a macOS .app so Finder, the Dock and
# Notification Center show the app icon. CFBundleURLTypes registers `otherwise-vigia://` (the
# block is otherwise_link::register::macos_url_types; a core test keeps them equal).
# Usage: script/bundle-macos.sh [debug|release]   (default: release; builds first)
#        UNIVERSAL=1 script/bundle-macos.sh       (release for Apple silicon + Intel, joined with lipo)
set -euo pipefail
profile="${1:-release}"
root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$root"
target="${CARGO_TARGET_DIR:-target}"
if [ "${UNIVERSAL:-0}" = 1 ]; then
  profile=release
  for t in aarch64-apple-darwin x86_64-apple-darwin; do
    rustup target add "$t" >/dev/null
    cargo build -p vigia-app -p vigia-cli --release --target "$t"
  done
  mkdir -p "$target/release"
  for bin in vigia-app vigia; do
    lipo -create -output "$target/release/$bin" "$target/aarch64-apple-darwin/release/$bin" "$target/x86_64-apple-darwin/release/$bin"
  done
elif [ "$profile" = release ]; then cargo build -p vigia-app -p vigia-cli --release; else cargo build -p vigia-app -p vigia-cli; fi
version="$(cargo pkgid -p vigia-app | sed 's/.*[#@]//')"
app="$target/$profile/Vigía.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$target/$profile/vigia-app" "$app/Contents/MacOS/vigia-app"
cp "$target/$profile/vigia" "$app/Contents/MacOS/vigia"
cp crates/app/assets/logo/AppIcon.icns "$app/Contents/Resources/AppIcon.icns"
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Vigía</string>
  <key>CFBundleDisplayName</key><string>Vigía</string>
  <key>CFBundleIdentifier</key><string>io.github.otherwisetech.vigia-app</string>
  <key>CFBundleExecutable</key><string>vigia-app</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.productivity</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleURLTypes</key>
  <array>
    <dict>
      <key>CFBundleURLName</key><string>io.github.otherwisetech.vigia-app</string>
      <key>CFBundleTypeRole</key><string>Viewer</string>
      <key>CFBundleURLSchemes</key><array><string>otherwise-vigia</string></array>
    </dict>
  </array>
</dict>
</plist>
PLIST
plutil -lint -s "$app/Contents/Info.plist"
# Make LaunchServices pick up the (possibly changed) icon.
touch "$app"
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app" || true
echo "$app"
