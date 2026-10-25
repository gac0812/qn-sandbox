#!/bin/sh
# Download the third-party assets that are not committed (see CREDITS):
#   fonts/   — OFL fonts from github.com/google/fonts
#   samples/ — CC0 VCSL instrument samples (copies in github.com/Vincentwei1021/mg-styles-15), decoded to 48 kHz WAV
#   node_modules/three — three.js r170 (MIT)
set -e
cd "$(dirname "$0")"
mkdir -p fonts
G=https://raw.githubusercontent.com/google/fonts/main/ofl
curl -fsSL -o fonts/NotoSansSC.ttf    "$G/notosanssc/NotoSansSC%5Bwght%5D.ttf"
curl -fsSL -o fonts/ZCOOLKuaiLe.ttf   "$G/zcoolkuaile/ZCOOLKuaiLe-Regular.ttf"
curl -fsSL -o fonts/Fredoka.ttf       "$G/fredoka/Fredoka%5Bwdth%2Cwght%5D.ttf"
curl -fsSL -o fonts/JetBrainsMono.ttf "$G/jetbrainsmono/JetBrainsMono%5Bwght%5D.ttf"
TMP=$(mktemp -d)
git clone --depth 1 --filter=blob:none --sparse -q https://github.com/Vincentwei1021/mg-styles-15 "$TMP/mg"
(cd "$TMP/mg" && git sparse-checkout set lib/audio/samples)
for d in kalimba marimba glock harp vibraphone cabasa; do
  mkdir -p "samples/$d"
  for f in "$TMP/mg/lib/audio/samples/$d"/*.flac; do
    ffmpeg -loglevel error -y -i "$f" -ar 48000 -ac 2 -c:a pcm_s16le "samples/$d/$(basename "${f%.flac}").wav"
  done
done
rm -rf "$TMP"
npm i --no-audit --no-fund
echo "assets ready"
