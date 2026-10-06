#!/usr/bin/env bash
# web/icons/icon.svg から PWA 用の RGBA PNG を作る。
set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
src="$root/web/icons/icon.svg"
out="$root/web/public"
mkdir -p "$out"

render() {
  local size=$1
  local dest=$2
  magick "$src" -resize "${size}x${size}" -depth 8 -type TrueColorAlpha -define png:color-type=6 "$dest"
}

render 192 "$out/icon-192.png"
render 512 "$out/icon-512.png"
render 180 "$out/apple-touch-icon.png"
