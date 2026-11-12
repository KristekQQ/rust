#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export JAVA_HOME="${JAVA_HOME:-/opt/homebrew/opt/openjdk}"
export PATH="$JAVA_HOME/bin:$PATH"
mkdir -p build/testclasses
javac -d build/testclasses src/cz/honza/queststream/LocalServer.java src/cz/honza/queststream/FragmentedMp4.java test/BrowserFixture.java test/cz/honza/queststream/FragmentedMp4Test.java
ffmpeg -hide_banner -loglevel error -y -f lavfi -i testsrc2=size=320x240:rate=30 -frames:v 360 -c:v libx264 -profile:v baseline -pix_fmt yuv420p -x264-params 'bframes=0:keyint=30:min-keyint=30:scenecut=0:aud=1:repeat-headers=1' -f h264 build/fixture.h264
audio_args=()
if [[ "${QUEST_STREAM_FIXTURE_AUDIO:-0}" == 1 ]]; then
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i 'sine=frequency=880:sample_rate=48000:duration=12' -ac 2 -c:a aac -profile:a aac_low -b:a 128k -f adts build/fixture.aac
  audio_args=(build/fixture.aac)
fi
java -cp build/testclasses cz.honza.queststream.FragmentedMp4Test build/fixture.h264 build/fixture.mp4 360 "${audio_args[@]}"
exec java -cp build/testclasses BrowserFixture "$(pwd)"
