#!/bin/zsh
# Renders assets/icon.svg into assets/AppIcon.icns (all macOS sizes) with headless Chrome.
# Run it after editing the SVG; bundle.sh only copies the .icns.
set -e
cd "$(dirname "$0")/.."
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
TMP=$(mktemp -d)
SET="$TMP/AppIcon.iconset"
mkdir -p "$SET"
cp assets/icon.svg "$TMP/icon.svg"
print '<!doctype html><style>html,body{margin:0;background:transparent}img{display:block;width:100vw;height:100vh}</style><img src="icon.svg">' > "$TMP/icon.html"
for px in 16 32 64 128 256 512 1024; do
  "$CHROME" --headless=new --force-device-scale-factor=1 --window-size=$px,$px --default-background-color=00000000 \
    --hide-scrollbars --screenshot="$TMP/$px.png" "file://$TMP/icon.html" 2>/dev/null
done
for px in 16 32 128 256 512; do
  cp "$TMP/$px.png" "$SET/icon_${px}x${px}.png"
  cp "$TMP/$((px * 2)).png" "$SET/icon_${px}x${px}@2x.png"
done
iconutil -c icns "$SET" -o assets/AppIcon.icns
rm -rf "$TMP"
echo "Wrote assets/AppIcon.icns"
