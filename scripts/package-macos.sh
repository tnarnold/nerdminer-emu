#!/usr/bin/env bash
# Builds "NerdMiner Emulator.app" (universal: Apple Silicon + Intel) and a .dmg installer.
#
#   scripts/package-macos.sh            -> dist/NerdMiner-Emulator-<versão>-macos-universal.dmg
#
# Optional signing/notarization (needs an Apple Developer ID):
#   MACOS_SIGN_IDENTITY="Developer ID Application: Nome (TEAMID)"
#   MACOS_NOTARY_PROFILE="perfil-do-notarytool"   (xcrun notarytool store-credentials)
# Without them the app is ad-hoc signed.
set -euo pipefail

cd "$(dirname "$0")/.."
APP_NAME="NerdMiner Emulator"
BIN="nerdminer-emu"
BUNDLE_ID="com.github.nerdminer-emu"
VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
DIST="dist"
APP="$DIST/$APP_NAME.app"
DMG="$DIST/NerdMiner-Emulator-$VERSION-macos-universal.dmg"

echo "==> Compilando $BIN $VERSION (arm64 + x86_64)"
rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --release --target aarch64-apple-darwin
cargo build --release --target x86_64-apple-darwin

echo "==> Montando $APP"
rm -rf "$APP" "$DMG"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create -output "$APP/Contents/MacOS/$BIN" \
  "target/aarch64-apple-darwin/release/$BIN" \
  "target/x86_64-apple-darwin/release/$BIN"
cp assets/icon/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"

cat > "$APP/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP_NAME</string>
  <key>CFBundleDisplayName</key><string>$APP_NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>$BIN</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSHumanReadableCopyright</key><string>MIT License. Interface baseada no NerdMiner v2 (Bitmaker, MIT).</string>
</dict>
</plist>
EOF

echo "==> Assinando"
if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  codesign --force --deep --options runtime --timestamp --sign "$MACOS_SIGN_IDENTITY" "$APP"
else
  codesign --force --deep --sign - "$APP"   # ad-hoc
fi
codesign --verify --deep --strict "$APP"

echo "==> Criando $DMG"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "$APP_NAME" -srcfolder "$STAGE" -ov -format UDZO "$DMG" >/dev/null

if [[ -n "${MACOS_SIGN_IDENTITY:-}" ]]; then
  codesign --force --sign "$MACOS_SIGN_IDENTITY" "$DMG"
fi
if [[ -n "${MACOS_NOTARY_PROFILE:-}" ]]; then
  echo "==> Notarizando"
  xcrun notarytool submit "$DMG" --keychain-profile "$MACOS_NOTARY_PROFILE" --wait
  xcrun stapler staple "$DMG"
fi

echo "==> Pronto: $DMG"
lipo -archs "$APP/Contents/MacOS/$BIN"
du -h "$DMG"
