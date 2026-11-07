# Quest Stream

Integrated project tool: `tools/quest-stream/`. Android package: `cz.honza.queststream`; launch activity: `.MainActivity`. Operational agent guidance: [quest-stream skill](../../.agents/skills/quest-stream/SKILL.md).

Standalone, video-only Android companion for Quest. No engine changes, third scene render, CPU pixel readback, or software video encoder. Horizon OS MediaProjection feeds the hardware AVC encoder's input Surface; a local HTTP server forwards encoded access units in fragmented MP4. The browser uses MediaSource to decode the stream. Compressed packets are copied for muxing/network transport; this is not an end-to-end zero-copy claim.

## Build and install

Requires JDK 17+, Android SDK platform/build-tools 35 (the script defaults to macOS SDK and Homebrew OpenJDK). Override `JAVA_HOME`, `ANDROID_SDK_ROOT` if needed. No Gradle, external Android dependencies, downloads, or engine build.

```sh
./build.sh
$HOME/Library/Android/sdk/platform-tools/adb -s DEVICE install -r build/QuestStream.apk
$HOME/Library/Android/sdk/platform-tools/adb -s DEVICE shell am start -n cz.honza.queststream/.MainActivity
```

APK is locally development-signed; preserve `build/development.keystore` for updates. If the key is lost, uninstalling the existing app is needed before installing a differently signed build.

1. Open **Quest Stream** in the headset's installed/unknown-source apps.
2. Choose 720p/30fps/3Mb/s or 540p/24fps/2Mb/s; press **Povolit přenos** and accept the system capture prompt. Choose the entire display if offered app-only sharing.
3. Open the displayed `http://QUEST_IP:8088/?token=…` on a computer on the same LAN. Current desktop Chrome/Edge is the initial viewer target.
4. Press **Sledovat**, then return to the VR game. The URL includes the required per-session access key.
5. **Odpojit** or closing/reloading the viewer only disconnects that browser. Click **Sledovat** again using the same link while headset sharing is active. The headset **Ukončit přenos** button, notification action, encoder error or OS capture revocation ends the session. Concurrent OS casting can revoke this session.

The codec and capture Surface start immediately after system consent, so a delayed first browser connection does not reuse an expired consent token. A session allows two browser viewers with independent bounded sixteen-packet queues and bounded socket send buffers. Each viewer begins at the next key frame with a fresh MP4 init segment. A slow viewer is disconnected without ending capture or affecting the other viewer. A static screen is requested to repeat at most after one second to detect dead connections. Stream profile is a requested cap; actual codec/compositor behavior must be checked on the device. There is no reconnect using a consumed projection token, recording, cloud relay, or audio capture. Encoding continues until sharing is stopped in the headset, including when no browser is viewing.

Use only trusted Wi-Fi: the random URL key controls access, but HTTP does not encrypt the video. This MVP targets IPv4 LANs and two concurrent viewers. Quest compositor output may differ from the full per-eye view, and protected content may be excluded. Operation alongside native games and immersive WebXR apps, profile changes, framing/scaling and long-run behavior require separate headset acceptance.

## Wi-Fi and laptop-local viewing

Find the current Quest address over USB with `adb -s USB_SERIAL shell ip -f inet addr show wlan0`. Enable Wi-Fi ADB with `adb -s USB_SERIAL tcpip 5555`, then `adb connect QUEST_IP:5555`. Confirm `adb -s QUEST_IP:5555 shell getprop ro.product.model` succeeds before unplugging. Subsequent commands target this network serial.

`adb -s QUEST_IP:5555 forward tcp:8088 tcp:8088` lets the laptop open `http://localhost:8088/?token=CURRENT_TOKEN` while the headset connection remains wireless. Read the actual token from the app after consent; do not reuse historical links. If an existing ADB daemon cannot connect despite a successful host TCP probe, an isolated `adb -P 5038 connect QUEST_IP:5555` may help; include the same `-P 5038` in subsequent commands and the awake helper. Do not restart other tasks' servers. Direct Quest-IP URLs remain available on the LAN.

## Verification

```sh
test/verify-local.sh
./build.sh
```

The JVM checks validate MP4 boxes and offsets, AVC conversion, timestamps and a real baseline AVC fixture. ffprobe validates metadata and all 60 decoded frame hashes match the source. HTTP regression checks also verify that shutdown closes both active and queued peers. `test/serve-fixture.sh` prints a temporary local URL for browser QA using the shipped viewer and server with synthetic video; it exits after ten minutes. These checks do not prove a live headset stream or its performance.

Physical check on 2026-11-07: Quest 3 hardware encoder `c2.qti.avc.encoder` streamed 1280×720 over Wi-Fi to the in-app browser. Same-token page reload resumed decoding; a second concurrent viewer decoded, and the first continued after closing the second. The keep-awake helper restored the sensor while capture was inactive, then applied the override for active capture; the headset reported Awake. Actual doffing duration and long-run endurance were not independently confirmed. Desktop Chrome playback had also been observed; Windows/Linux receiver compatibility was not physically tested.

For remaining device acceptance: test permission cancellation/revocation, both profiles, headset app switching/native games/WebXR, direct LAN viewing, third viewer rejection, slow/disconnected network cleanup, off-face continuity/sensor restoration and starting a fresh session. Record hardware encoder name from the app status. Compare the same deterministic VR workload with casting off/on at matched quality, device refresh rate, thermal state and power mode; measure frame times/tails, battery/thermal effects and stream latency. Hardware encoding still costs capture, memory bandwidth, encoder and Wi-Fi work; no FPS/energy savings are claimed until measured.

## References

- [Meta MediaProjection](https://developers.meta.com/vr/documentation/native/native-media-projection/) describes compositor-fed Surface capture and consent/revocation.
- [Android MediaProjection](https://developer.android.com/media/grow/media-projection) covers foreground-service and session-token lifecycle.

## Keep the headset awake off your face

With an authorized Wi-Fi ADB connection, run `python3 keep-awake.py --serial QUEST_IP:5555` (use `--adb /path/to/adb --adb-port 5038` when needed). The helper polls MediaProjection ownership every two seconds, enables Meta’s `prox_close` override only while Quest Stream owns capture and restores the physical sensor with `automation_disable` when capture stops or the helper exits. A lost ADB connection prevents restoration until it reconnects; reboot also resets this temporary override. This is a development-device helper, not an Android app permission.

The helper intentionally keeps the headset active when removed during sharing. Stop sharing in Quest Stream after use. References: [Meta off-face setup and restoration](https://developers.meta.com/vr/documentation/unity/meta-xr-operator/quest/#test-with-the-headset-off-your-face).
