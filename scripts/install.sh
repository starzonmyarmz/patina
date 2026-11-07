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
