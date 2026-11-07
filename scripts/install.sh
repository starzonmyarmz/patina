#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

cargo xtask bundle patina --release

dest="$HOME/Library/Audio/Plug-Ins"
mkdir -p "$dest/VST3" "$dest/CLAP"
for kind in vst3:VST3 clap:CLAP; do
  ext=${kind%%:*}
  dir=${kind##*:}
  rm -rf "$dest/$dir/Patina.$ext"
  cp -R "target/bundled/Patina.$ext" "$dest/$dir/"
  codesign --force --deep --sign - "$dest/$dir/Patina.$ext"
  echo "Installed $dest/$dir/Patina.$ext"
done

cargo build --release -p patina-app
app="target/bundled/Patina.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/patina-app "$app/Contents/MacOS/Patina"

icons="target/bundled/Patina.iconset"
rm -rf "$icons"
mkdir -p "$icons"
qlmanage -t -s 1024 -o target/bundled app/assets/icon.svg >/dev/null
for size in 16 32 128 256 512; do
  sips -z $size $size target/bundled/icon.svg.png --out "$icons/icon_${size}x${size}.png" >/dev/null
  sips -z $((size * 2)) $((size * 2)) target/bundled/icon.svg.png --out "$icons/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$icons" -o "$app/Contents/Resources/Patina.icns"

version=$(sed -n 's/^version = "\(.*\)"/\1/p' app/Cargo.toml)
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>Patina</string>
  <key>CFBundleDisplayName</key><string>Patina</string>
  <key>CFBundleIdentifier</key><string>com.danmarino.patina.app</string>
  <key>CFBundleExecutable</key><string>Patina</string>
  <key>CFBundleIconFile</key><string>Patina</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$version</string>
  <key>CFBundleVersion</key><string>$version</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

codesign --force --deep --sign - "$app"
rm -rf /Applications/Patina.app
cp -R "$app" /Applications/
echo "Installed /Applications/Patina.app"
