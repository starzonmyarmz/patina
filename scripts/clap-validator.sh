#!/bin/sh
# Usage: scripts/clap-validator.sh target/bundled/Patina.clap
set -eu

version=0.4.1

if command -v clap-validator >/dev/null; then
  exec clap-validator validate "$@"
fi

dir=$(mktemp -d)
case "$(uname -s)" in
  Darwin)
    gh release download "$version" -R free-audio/clap-validator -p '*-macos-universal.zip' -D "$dir"
    unzip -q "$dir"/*.zip -d "$dir"
    tar -xzf "$dir"/*.tar.gz -C "$dir"
    bin="$dir/binaries/clap-validator"
    ;;
  MINGW* | MSYS* | CYGWIN*)
    gh release download "$version" -R free-audio/clap-validator -p '*-windows.zip' -D "$dir"
    unzip -q "$dir"/*.zip -d "$dir"
    bin="$dir/clap-validator.exe"
    ;;
  *)
    echo "No clap-validator download for $(uname -s)" >&2
    exit 1
    ;;
esac
exec "$bin" validate "$@"
