#!/bin/sh
# macOS 用の .app を作り、zip にまとめる。
# 使い方: sh scripts/bundle-macos.sh [ターゲット] [配布名]
#   例:   sh scripts/bundle-macos.sh aarch64-apple-darwin macos-arm64
set -eu

TARGET="${1:-aarch64-apple-darwin}"
NAME="${2:-macos-arm64}"
VERSION=$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)
BIN="target/$TARGET/release/flex-renamer"
APP="dist/Flex Renamer.app"

if [ ! -f "$BIN" ]; then
  echo "実行ファイルがありません: $BIN" >&2
  echo "先に cargo build --release -p renamer-gui --target $TARGET を実行してください" >&2
  exit 1
fi

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/flex-renamer"

cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Flex Renamer</string>
  <key>CFBundleDisplayName</key><string>Flex Renamer</string>
  <key>CFBundleIdentifier</key><string>com.example.flexrenamer</string>
  <key>CFBundleExecutable</key><string>flex-renamer</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>12.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# 署名なしでも起動できるよう、アドホック署名を付ける
codesign --force --deep --sign - "$APP"

ZIP="dist/flex-renamer-$NAME.zip"
rm -f "$ZIP"
ditto -c -k --keepParent "$APP" "$ZIP"
echo "作成: $ZIP"
