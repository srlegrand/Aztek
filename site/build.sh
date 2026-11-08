#!/bin/sh
# Assembles the website into a folder (default _site): the page plus the README's images.
#   sh site/build.sh [_site]
set -e
cd "$(dirname "$0")/.."
OUT=${1:-_site}
rm -rf "$OUT"
mkdir -p "$OUT/img"
cp site/index.html site/style.css "$OUT/"
cp docs/*.png docs/*.gif docs/*.jpg docs/*.mp4 "$OUT/img/"
cp app/icons/aztek-icon.png "$OUT/img/"
touch "$OUT/.nojekyll"
echo "Site in $OUT"
