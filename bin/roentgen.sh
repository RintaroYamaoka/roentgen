#!/usr/bin/env bash
# roentgen のランチャー。バイナリの探し順:
#   1) $ROENTGEN_BIN  2) ~/.roentgen/bin/roentgen(この版)  3) このプラグインの target/release  4) cargo があればビルド  5) GitHub Releases から落とす
set -euo pipefail
HERE="$(cd "$(dirname "$(readlink -f "${BASH_SOURCE[0]}")")/.." && pwd)"
VERSION="$(grep -m1 '^version' "$HERE/Cargo.toml" | sed 's/.*"\(.*\)"/\1/')"
REPO="${ROENTGEN_REPO:-RintaroYamaoka/roentgen}"
BIN_DIR="${ROENTGEN_HOME:-$HOME/.roentgen}/bin"
BIN="$BIN_DIR/roentgen-$VERSION"

run() { exec "$1" "$@" ; }
if [ -n "${ROENTGEN_BIN:-}" ] && [ -x "$ROENTGEN_BIN" ]; then exec "$ROENTGEN_BIN" "$@"; fi
if [ -x "$BIN" ]; then exec "$BIN" "$@"; fi
if [ -x "$HERE/target/release/roentgen" ]; then exec "$HERE/target/release/roentgen" "$@"; fi
if command -v cargo >/dev/null 2>&1 && [ -f "$HERE/Cargo.toml" ]; then
  echo "roentgen: バイナリが無いのでビルドする(初回のみ・数分)" >&2
  (cd "$HERE" && cargo build --release -q) && exec "$HERE/target/release/roentgen" "$@"
fi
# Releases から落とす
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64) ASSET="roentgen-linux-x86_64" ;; Linux-aarch64) ASSET="roentgen-linux-aarch64" ;;
  Darwin-arm64) ASSET="roentgen-macos-aarch64" ;; Darwin-x86_64) ASSET="roentgen-macos-x86_64" ;;
  *) echo "roentgen: この OS 用のバイナリが無い($(uname -s)-$(uname -m))。cargo を入れてビルドするか、ROENTGEN_BIN を指定する" >&2; exit 2 ;;
esac
mkdir -p "$BIN_DIR"
URL="https://github.com/$REPO/releases/download/v$VERSION/$ASSET"
echo "roentgen: $URL を取得" >&2
if command -v gh >/dev/null 2>&1 && gh release download "v$VERSION" -R "$REPO" -p "$ASSET" -O "$BIN" --clobber 2>/dev/null; then :; else curl -fsSL "$URL" -o "$BIN"; fi
chmod +x "$BIN"
exec "$BIN" "$@"
