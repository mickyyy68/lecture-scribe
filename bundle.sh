#!/bin/zsh
# Builds "Lecture Scribe.app" and installs it to ~/Applications.
set -e
cd "$(dirname "$0")"
# The macOS 27 SDK's libSystem.tbd isn't readable by the current linker; build against 26.5.
[ -d /Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk ] && export SDKROOT=/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk
cargo build --release
APP="target/Lecture Scribe.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp target/release/lecture-scribe "$APP/Contents/MacOS/lecture-scribe"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>Lecture Scribe</string>
  <key>CFBundleDisplayName</key><string>Lecture Scribe</string>
  <key>CFBundleIdentifier</key><string>local.lecture-scribe</string>
  <key>CFBundleExecutable</key><string>lecture-scribe</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>13.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Lecture Scribe records the lecture so it can transcribe it on this Mac.</string>
</dict></plist>
PLIST
codesign --force --deep -s - "$APP"
mkdir -p ~/Applications
rm -rf ~/Applications/"Lecture Scribe.app"
cp -R "$APP" ~/Applications/
echo "Installed ~/Applications/Lecture Scribe.app"
