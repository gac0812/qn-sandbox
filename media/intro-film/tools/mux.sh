#!/bin/sh
# mux.sh video.mp4 mix.wav out.mp4 — two-pass loudnorm to -14 LUFS, true peak <= -1.5 dBTP
set -e
V=$1; A=$2; O=$3
J=$(ffmpeg -hide_banner -nostats -i "$A" -af loudnorm=I=-14:TP=-2.0:LRA=11:print_format=json -f null - 2>&1 | sed -n '/^{/,/^}/p')
g() { echo "$J" | grep "\"$1\"" | sed 's/.*: "\(.*\)".*/\1/'; }
F="loudnorm=I=-14:TP=-2.0:LRA=11:measured_I=$(g input_i):measured_TP=$(g input_tp):measured_LRA=$(g input_lra):measured_thresh=$(g input_thresh):offset=$(g target_offset):linear=true"
ffmpeg -loglevel error -y -i "$V" -i "$A" -map 0:v -map 1:a -c:v copy -af "$F,aresample=48000" -c:a aac -b:a 256k -shortest "$O"
ffmpeg -hide_banner -nostats -i "$O" -map 0:a -af ebur128=peak=true -f null - 2>&1 | grep -A12 "Summary:" | grep -E "I:|Peak:"
