#!/bin/sh
# One-command rebuild of the qn sandbox film.
#   sh build.sh            # picture + sound + subtitles + poster
#   AUDIO_ONLY=1 sh build.sh   # re-mix sound only, reuse out/final/video.mp4
#   MINIMAX_KEY=... sh build.sh # also re-synthesise changed narration lines
# Needs: Node 20+ with Playwright + Chromium, ffmpeg, Python 3 with numpy, scipy (io only), pillow.
# First run: sh fetch-assets.sh (fonts, CC0 samples, three.js). Narration WAVs are not committed:
# set MINIMAX_KEY to synthesise them from film/vo/lines.json.
set -e
cd "$(dirname "$0")"
OUT=out/final
mkdir -p "$OUT"
# narration (needs MINIMAX_KEY in the environment; cached lines are reused)
if [ -n "$MINIMAX_KEY" ]; then python3 tools/tts.py film/vo/lines.json film/vo; fi
if [ -z "$AUDIO_ONLY" ]; then
  node tools/video.mjs film/index.html "$OUT/video.mp4" --fps 30 --workers "${WORKERS:-4}"
fi
python3 audio/mix.py "$OUT/events.json" "$OUT/mix.wav"
sh tools/mux.sh "$OUT/video.mp4" "$OUT/mix.wav" qn-sandbox.mp4
python3 tools/srt.py "$OUT/events.json" qn-sandbox.srt
DUR=$(python3 -c "import json;print(json.load(open('$OUT/events.json'))['DUR'])")
POSTER_T=$(python3 -c "print($DUR - 1.5)")
node tools/still.mjs film/index.html "$OUT/poster.png" "$POSTER_T"
python3 -c "from PIL import Image; Image.open('$OUT/poster.png').convert('RGB').save('poster.jpg', quality=92)"
echo "done: qn-sandbox.mp4  qn-sandbox.srt  poster.jpg"
