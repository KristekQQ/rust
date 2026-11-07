#!/bin/sh
# Requires a JDK, ffmpeg with libx264, ffprobe, and standard POSIX tools.
set -eu
root=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT HUP INT TERM
javac -d "$tmp/classes" "$root/src/cz/honza/queststream/FragmentedMp4.java" \
  "$root/test/cz/honza/queststream/FragmentedMp4Test.java"
ffmpeg -hide_banner -loglevel error -f lavfi -i testsrc2=size=320x240:rate=30 \
  -frames:v 60 -c:v libx264 -profile:v baseline -pix_fmt yuv420p \
  -x264-params 'bframes=0:keyint=30:min-keyint=30:scenecut=0:aud=1:repeat-headers=1' \
  -f h264 "$tmp/input.h264"
java -ea -cp "$tmp/classes" cz.honza.queststream.FragmentedMp4Test "$tmp/input.h264" "$tmp/output.mp4"
probe=$(ffprobe -v error -select_streams v:0 -count_frames \
  -show_entries stream=codec_name,width,height,nb_read_frames,time_base -of default=noprint_wrappers=1 "$tmp/output.mp4")
printf '%s\n' "$probe"
printf '%s\n' "$probe" | grep -qx 'codec_name=h264'
printf '%s\n' "$probe" | grep -qx 'width=320'
printf '%s\n' "$probe" | grep -qx 'height=240'
printf '%s\n' "$probe" | grep -qx 'nb_read_frames=60'
printf '%s\n' "$probe" | grep -qx 'time_base=1/1000000'
ffmpeg -hide_banner -loglevel error -i "$tmp/input.h264" -f framemd5 "$tmp/expected.md5"
ffmpeg -hide_banner -loglevel error -i "$tmp/output.mp4" -f framemd5 "$tmp/actual.md5"
awk -F, '!/^#/ {gsub(/ /,"",$6); print $6}' "$tmp/expected.md5" > "$tmp/expected.hashes"
awk -F, '!/^#/ {gsub(/ /,"",$6); print $6}' "$tmp/actual.md5" > "$tmp/actual.hashes"
cmp "$tmp/expected.hashes" "$tmp/actual.hashes"
printf '%s\n' 'PASS ffprobe metadata and all 60 decoded frame hashes match Annex B source'
