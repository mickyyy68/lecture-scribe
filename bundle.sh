#!/bin/zsh
# Builds "Transcribe Lecture.app" in target/.
#
#   ./bundle.sh             build the app
#   ./bundle.sh --install   build it and copy it to ~/Applications (refuses while the app is open)
#
# Optional environment:
#   SDKROOT=…        an older macOS SDK, if the linker can't read the newest one
#   SIGN_IDENTITY=…  a Developer ID to sign with (default: ad-hoc, for this Mac only)
#   BUNDLE_ID=…      the app's bundle identifier
set -e
cd "$(dirname "$0")"
NAME="Transcribe Lecture"
BUNDLE_ID="${BUNDLE_ID:-io.github.mickyyy68.transcribe-lecture}"
VERSION=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -1)

cargo build --release
APP="target/$NAME.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/transcribe-lecture "$APP/Contents/MacOS/transcribe-lecture"
cp assets/AppIcon.icns "$APP/Contents/Resources/AppIcon.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>$NAME</string>
  <key>CFBundleDisplayName</key><string>$NAME</string>
  <key>CFBundleIdentifier</key><string>$BUNDLE_ID</string>
  <key>CFBundleExecutable</key><string>transcribe-lecture</string>
  <key>CFBundleIconFile</key><string>AppIcon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>$NAME records the lecture so it can transcribe it on this Mac.</string>
</dict></plist>
PLIST
codesign --force --deep -s "${SIGN_IDENTITY:--}" "$APP"
echo "Built $APP"

if [[ "$1" == "--install" ]]; then
  # Replacing the app while it records would lose the lecture.
  if pgrep -f "$NAME.app/Contents/MacOS/" >/dev/null; then
    echo "$NAME is open. Quit it (after the lecture) and run this again." >&2
    exit 1
  fi
  mkdir -p ~/Applications
  rm -rf ~/Applications/"$NAME.app"
  cp -R "$APP" ~/Applications/
  echo "Installed ~/Applications/$NAME.app"
fi
