#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

cargo xtask bundle thousand_synths --release

dest="$HOME/Library/Audio/Plug-Ins"
mkdir -p "$dest/VST3" "$dest/CLAP"
for kind in vst3:VST3 clap:CLAP; do
  ext=${kind%%:*}
  dir=${kind##*:}
  rm -rf "$dest/$dir/1000 Synths.$ext"
  cp -R "target/bundled/1000 Synths.$ext" "$dest/$dir/"
  codesign --force --deep --sign - "$dest/$dir/1000 Synths.$ext"
  echo "Installed $dest/$dir/1000 Synths.$ext"
done
