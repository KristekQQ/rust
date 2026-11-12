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

ffmpeg -hide_banner -loglevel error -f lavfi -i 'sine=frequency=880:sample_rate=48000:duration=1.8' \
  -ac 2 -c:a aac -profile:a aac_low -b:a 128k -f adts "$tmp/audio.aac"
java -ea -cp "$tmp/classes" cz.honza.queststream.FragmentedMp4Test "$tmp/input.h264" "$tmp/av.mp4" 60 "$tmp/audio.aac"
probe=$(ffprobe -v error -select_streams a:0 -show_entries stream=codec_name,sample_rate,channels,start_time -of default=noprint_wrappers=1 "$tmp/av.mp4")
printf '%s\n' "$probe"
printf '%s\n' "$probe" | grep -qx 'codec_name=aac'
printf '%s\n' "$probe" | grep -qx 'sample_rate=48000'
printf '%s\n' "$probe" | grep -qx 'channels=2'
printf '%s\n' "$probe" | grep -qx 'start_time=0.120000'
ffmpeg -hide_banner -loglevel error -i "$tmp/audio.aac" -map 0:a:0 -f s16le "$tmp/expected.pcm"
ffmpeg -hide_banner -loglevel error -i "$tmp/av.mp4" -map 0:a:0 -f s16le "$tmp/actual.pcm"
cmp "$tmp/expected.pcm" "$tmp/actual.pcm"
ffmpeg -hide_banner -loglevel error -i "$tmp/av.mp4" -map 0:v:0 -f framemd5 "$tmp/av.md5"
awk -F, '!/^#/ {gsub(/ /,"",$6); print $6}' "$tmp/av.md5" > "$tmp/av.hashes"
cmp "$tmp/expected.hashes" "$tmp/av.hashes"
printf '%s\n' 'PASS interleaved AVC/AAC: shared-clock 120ms offset, exact decoded PCM and all video frames'
