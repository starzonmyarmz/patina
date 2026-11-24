#!/bin/sh
set -eu
cd "$(dirname "$0")/.."

crate=patina
name=$(sed -n 's/^name = "\(.*\)"/\1/p' bundler.toml)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml)
build=$(git rev-parse --short HEAD)
if [ -n "$(git status --porcelain)" ]; then
  build="$build.dirty"
fi
export PLUGIN_VERSION="$version-dev+$build"

cargo xtask bundle "$crate" --release

dest="$HOME/Library/Audio/Plug-Ins"
mkdir -p "$dest/VST3" "$dest/CLAP"
for kind in vst3:VST3 clap:CLAP; do
  ext=${kind%%:*}
  dir=${kind##*:}
  rm -rf "$dest/$dir/$name.$ext"
  cp -R "target/bundled/$name.$ext" "$dest/$dir/"
  codesign --force --deep --sign - "$dest/$dir/$name.$ext"
  echo "Installed $dest/$dir/$name.$ext"
done
if [ -x scripts/install-extra.sh ]; then
  scripts/install-extra.sh
fi
echo "Version $PLUGIN_VERSION. Restart the host to load it."
