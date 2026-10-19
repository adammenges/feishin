#!/bin/sh
set -eu
cd "$(dirname "$0")/.."
if [ "${1:-}" != "--skip-build" ]; then
    cargo build --release --locked -p feishin-gpui
fi
package=target/feishin-linux
mkdir -p "$package/bin" "$package/share/applications" "$package/share/icons/hicolor/512x512/apps"
cp target/release/feishin "$package/bin/feishin"
licenses="$package/share/licenses/feishin"
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
cp assets/icons/512x512.png "$package/share/icons/hicolor/512x512/apps/feishin.png"
cat > "$package/share/applications/feishin.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Feishin
Comment=Music player for Navidrome, Subsonic and Jellyfin
Exec=feishin
Icon=feishin
Terminal=false
Categories=AudioVideo;Audio;Player;
DESKTOP
tar -czf target/feishin-linux.tar.gz -C target feishin-linux
