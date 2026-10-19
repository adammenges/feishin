#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" != "--skip-build" ]; then
    cargo build --release --locked -p feishin-gpui -p feishin-audio-capture
fi
version=$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json,sys; print(next(p["version"] for p in json.load(sys.stdin)["packages"] if p["name"] == "feishin-gpui").split("-")[0])')
bundle=target/Feishin.app
mkdir -p "$bundle/Contents/MacOS" "$bundle/Contents/Resources"
install -m 755 target/release/feishin "$bundle/Contents/MacOS/feishin.new"
mv -f "$bundle/Contents/MacOS/feishin.new" "$bundle/Contents/MacOS/feishin"
install -m 755 target/release/feishin-audio-capture "$bundle/Contents/MacOS/feishin-audio-capture.new"
mv -f "$bundle/Contents/MacOS/feishin-audio-capture.new" "$bundle/Contents/MacOS/feishin-audio-capture"
licenses="$bundle/Contents/Resources/licenses"
mkdir -p "$licenses"
cp LICENSE "$licenses/Feishin.txt"
cp crates/feishin-core/assets/*LICENSE.txt crates/feishin-core/assets/*OFL.txt "$licenses/"
cp crates/feishin-gpui/assets/*LICENSE.txt crates/feishin-gpui/assets/*AUTHORS.txt "$licenses/"
cp crates/vendor/projectm-sys/LICENSE "$licenses/projectm-sys.txt"
cp crates/vendor/projectm-sys/libprojectM/src/libprojectM/COPYING "$licenses/projectM.txt"
cp crates/vendor/xattr/LICENSE-MIT "$licenses/xattr-MIT.txt"
cp crates/vendor/xattr/LICENSE-APACHE "$licenses/xattr-Apache.txt"
cp crates/vendor/cosmic-text/LICENSE-MIT "$licenses/cosmic-text-MIT.txt"
cp crates/vendor/cosmic-text/LICENSE-APACHE "$licenses/cosmic-text-Apache.txt"
cp crates/vendor/taglib/COPYING.LGPL "$licenses/TagLib-LGPL.txt"
cp crates/vendor/taglib/COPYING.MPL "$licenses/TagLib-MPL.txt"
cp crates/vendor/taglib/3rdparty/utfcpp/LICENSE "$licenses/utfcpp.txt"
cp docs/native/THIRD_PARTY.md "$licenses/"
iconset=target/Feishin.iconset
mkdir -p "$iconset"
for size in 16 32 128 256 512; do
    cp "assets/icons/${size}x${size}.png" "$iconset/icon_${size}x${size}.png"
    doubled=$((size * 2))
    cp "assets/icons/${doubled}x${doubled}.png" "$iconset/icon_${size}x${size}@2x.png"
done
iconutil -c icns "$iconset" -o "$bundle/Contents/Resources/icon.icns"
cat > "$bundle/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleExecutable</key><string>feishin</string>
<key>CFBundleIdentifier</key><string>org.jeffvli.feishin.rust</string>
<key>CFBundleName</key><string>Feishin</string>
<key>CFBundleDisplayName</key><string>Feishin</string>
<key>CFBundleVersion</key><string>$version</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleIconFile</key><string>icon.icns</string>
<key>NSHighResolutionCapable</key><true/>
<key>LSMinimumSystemVersion</key><string>12.3</string>
<key>NSScreenCaptureUsageDescription</key><string>Feishin uses system audio for visualizers when you enable capture.</string>
</dict></plist>
PLIST
# Rebuilding an existing bundle must replace both executable inodes and its seal.
codesign --force --deep --sign - "$bundle"
codesign --verify --deep --strict "$bundle"
printf '%s\n' "Built $bundle"
