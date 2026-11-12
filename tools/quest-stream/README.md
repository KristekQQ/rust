# Quest Stream

Integrated project tool: `tools/quest-stream/`. Android package: `cz.honza.queststream`; launch activity: `.MainActivity`. Operational agent guidance: [quest-stream skill](../../.agents/skills/quest-stream/SKILL.md).

Standalone Android video + optional playback-audio companion for Quest. No engine changes, third scene render, CPU pixel readback, or software video encoder. Horizon OS MediaProjection feeds the hardware AVC encoder's input Surface; a local HTTP server forwards encoded access units in fragmented MP4. Android AudioPlaybackCapture captures permitted game/media playback as 48 kHz stereo PCM and encodes AAC-LC at 128 kb/s. It never opens the microphone. The browser uses MediaSource to decode the stream. Compressed packets are copied for muxing/network transport; this is not an end-to-end zero-copy claim.

## Build and install

Requires JDK 17+, Android SDK platform/build-tools 35 (the script defaults to macOS SDK and Homebrew OpenJDK). Override `JAVA_HOME`, `ANDROID_SDK_ROOT` if needed. No Gradle, external Android dependencies, downloads, or engine build.

```sh
./build.sh
$HOME/Library/Android/sdk/platform-tools/adb -s DEVICE install -r build/QuestStream.apk
$HOME/Library/Android/sdk/platform-tools/adb -s DEVICE shell am start -n cz.honza.queststream/.MainActivity
```

APK is locally development-signed; preserve `build/development.keystore` for updates. If the key is lost, uninstalling the existing app is needed before installing a differently signed build.

1. Open **Quest Stream** in the headset's installed/unknown-source apps.
2. Choose 720p/30fps/3Mb/s or 540p/24fps/2Mb/s; press **Povolit přenos** and grant the Android audio permission (optional) and accept the system capture prompt. Denying audio permission keeps video working. Choose the entire display if offered app-only sharing.
3. Open the displayed `http://QUEST_IP:8088/?token=…` on a computer on the same LAN. Current desktop Chrome/Edge is the initial viewer target.
4. Once capture produces its first encoded frame, Quest Stream closes its own panel. The foreground service, system sharing indication and notification stop action remain active. Reopen Quest Stream to read the URL or stop capture; **Zavřít panel (přenos pokračuje)** closes only the panel again. Press **Sledovat** in the browser, then return to the VR game. The URL includes the required per-session access key.
5. **Odpojit** or closing/reloading the viewer only disconnects that browser. Click **Sledovat** again using the same link while headset sharing is active. The headset **Ukončit přenos** button, notification action, encoder error or OS capture revocation ends the session. Concurrent OS casting can revoke this session.

The codec and capture Surface start immediately after system consent, so a delayed first browser connection does not reuse an expired consent token. A session allows two browser viewers with independent bounded queues (96 packets for video + audio; 16 for video-only) and bounded socket send buffers. Each viewer begins at the next key frame with a fresh MP4 init segment. A slow viewer is disconnected without ending capture or affecting the other viewer. A static screen is requested to repeat at most after one second to detect dead connections. Stream profile is a requested cap; actual codec/compositor behavior must be checked on the device. There is no reconnect using a consumed projection token, recording or cloud relay. Encoding continues until sharing is stopped in the headset, including when no browser is viewing.

Use only trusted Wi-Fi: the random URL key controls access, but HTTP does not encrypt the video. This MVP targets IPv4 LANs and two concurrent viewers. Quest compositor output may differ from the full per-eye view, and protected content may be excluded. Operation alongside native games and immersive WebXR apps, profile changes, framing/scaling and long-run behavior require separate headset acceptance.

## Wi-Fi and laptop-local viewing

Find the current Quest address over USB with `adb -s USB_SERIAL shell ip -f inet addr show wlan0`. Enable Wi-Fi ADB with `adb -s USB_SERIAL tcpip 5555`, then `adb connect QUEST_IP:5555`. Confirm `adb -s QUEST_IP:5555 shell getprop ro.product.model` succeeds before unplugging. Subsequent commands target this network serial.

`adb -s QUEST_IP:5555 forward tcp:8088 tcp:8088` lets the laptop open `http://localhost:8088/?token=CURRENT_TOKEN` while the headset connection remains wireless. Read the actual token from the app after consent; do not reuse historical links. If an existing ADB daemon cannot connect despite a successful host TCP probe, an isolated `adb -P 5038 connect QUEST_IP:5555` may help; include the same `-P 5038` in subsequent commands and the awake helper. Do not restart other tasks' servers. Direct Quest-IP URLs remain available on the LAN.

## Receiver buffering

The viewer collects about 300 ms of contiguous video before playback (sparse/static capture may start after 600 ms on the next frame). It tolerates normal network bursts using playback rates from 0.97 to 1.03; only a backlog above 1.5 seconds or a real timeline gap requires a seek. Recovery keeps about 350 ms of reserve where the buffered range allows it. This trades a small receiver-side delay for fewer seek jumps, without raising capture resolution, encoder FPS or bitrate. Complete MP4 fragments use one MediaSource append per video frame or AAC access unit; old data is pruned at most once per media second.

These thresholds are initial receiver policy, not a measured Quest performance improvement. Game frame times and end-to-end video latency still require a matched hardware comparison.

## Verification

```sh
test/verify-local.sh
./build.sh
```

The JVM checks validate MP4 boxes and offsets, AVC conversion, timestamps and a real baseline AVC fixture. ffprobe validates metadata and all 60 decoded frame hashes match the source. An interleaved AVC/AAC fixture also verifies exact decoded stereo PCM and a deliberate 120 ms audio offset on the shared clock; AAC sample durations are 1024 samples at 48 kHz. HTTP regression checks also verify that shutdown closes both active and queued peers. `test/serve-fixture.sh` prints a temporary local URL for browser QA using the shipped viewer and server with synthetic video; it exits after ten minutes. The fixture lasts 12 seconds; `QUEST_STREAM_FIXTURE_AUDIO=1 test/serve-fixture.sh` includes a synthetic 880 Hz AAC tone for browser decoder/sync checks (not headset capture evidence). `QUEST_STREAM_FIXTURE_JITTER_MS=150 test/serve-fixture.sh` simulates a short delivery pause once per second followed by catch-up at the original source cadence. These checks do not prove a live headset stream or its performance.

Physical check on 2026-11-07: Quest 3 hardware encoder `c2.qti.avc.encoder` streamed 1280×720 over Wi-Fi to the in-app browser. Same-token page reload resumed decoding; a second concurrent viewer decoded, and the first continued after closing the second. The keep-awake helper restored the sensor while capture was inactive, then applied the override for active capture; the headset reported Awake. Actual doffing duration and long-run endurance were not independently confirmed. Desktop Chrome playback had also been observed; Windows/Linux receiver compatibility was not physically tested.

For remaining device acceptance: test audio permission denial, allowed/blocked source-app capture policy, game sound audibility and A/V sync on Quest, plus permission cancellation/revocation, both profiles, headset app switching/native games/WebXR, direct LAN viewing, third viewer rejection, slow/disconnected network cleanup, off-face continuity/sensor restoration and starting a fresh session. Record hardware encoder name from the app status. Compare the same deterministic VR workload with casting off/on at matched quality, device refresh rate, thermal state and power mode; measure frame times/tails, battery/thermal effects and stream latency. Hardware encoding still costs capture, memory bandwidth, encoder and Wi-Fi work; no FPS/energy savings are claimed until measured.

## References

- [Meta MediaProjection](https://developers.meta.com/vr/documentation/native/native-media-projection/) describes compositor-fed Surface capture and consent/revocation.
- [Android MediaProjection](https://developer.android.com/media/grow/media-projection) covers foreground-service and session-token lifecycle.

## Keep the headset awake off your face

With an authorized Wi-Fi ADB connection, run `python3 keep-awake.py --serial QUEST_IP:5555` (use `--adb /path/to/adb --adb-port 5038` when needed). The helper polls MediaProjection ownership every two seconds, enables Meta’s `prox_close` override only while Quest Stream owns capture and restores the physical sensor with `automation_disable` when capture stops or the helper exits. A lost ADB connection prevents restoration until it reconnects; reboot also resets this temporary override. This is a development-device helper, not an Android app permission.

To keep one current native game process awake as well, use the **same single helper** with `--keep-app cz.kristek.neon.nativevr --app-pid CURRENT_PID`. Capture remains supported: the override stays active while Quest Stream owns capture **or** that exact package PID exists. Without `--app-pid`, the helper captures one current PID on its first successful check. Once that process exits, the game lease ends permanently; a restarted process needs a newly selected lease. Going Home does not end a lease if Android keeps the process alive. There is no permanent power setting or app reinstall. On shutdown even an initially unknown/adopted override is restored. Stop an old helper gracefully and wait for its restoration to finish before starting a replacement; never run competing helpers. Test policy changes with `python3 test/keep_awake_test.py`. Physical off-face Awake/Wi-Fi acceptance is separate from these tests.


For an explicitly authorized development session across native app restarts,
add `--follow-app` to `--keep-app cz.kristek.neon.nativevr`. The single helper
follows new PIDs for that package and holds the override for up to 30 seconds
between processes. If the app stays stopped and capture is inactive, it restores
the sensor; a later app launch reactivates the override. This does not require
an APK change. Stop the helper to end this development session. A disconnected
ADB link still prevents restoration until reconnection; this is not a permanent
headset setting and does not solve room-tracking loss.

The helper intentionally keeps the headset active when removed during sharing. Stop sharing in Quest Stream after use. References: [Meta off-face setup and restoration](https://developers.meta.com/vr/documentation/unity/meta-xr-operator/quest/#test-with-the-headset-off-your-face).

After an ADB polling failure the helper marks its proximity state unknown. On reconnection it reapplies the current capture/native-app policy, even if a headset reboot cleared the old override. The existing app lease remains intact during an outage. Wi-Fi ADB itself may need re-enabling over USB after a headset reboot.

## Playback audio policy and failure behavior

Both AVC Surface timestamps and AudioRecord timestamps use the monotonic clock with one session origin. AAC uses AudioRecord's frame-position timestamp when available and a sample-count clock anchored at recording start otherwise. Late readers wait for a video key frame and omit audio preceding that key frame. PCM reads, codec polls and per-viewer queue offers are nonblocking; capture/encoding stays on the companion worker, independent of game threads.

Android only permits capture of GAME, MEDIA or UNKNOWN playback in the same user profile when the source app permits `ALLOW_CAPTURE_BY_ALL`. Protected, voice-call and policy-blocked sounds may remain silent even when the AAC track is active. The browser reports an active AAC track, not proof of audible source content. Audio permission/setup failure falls back to video-only. If audio fails during streaming, current readers disconnect; click **Sledovat** to receive a new video-only init segment without renewing projection consent. The service retains the `mediaProjection` foreground type; no microphone capture/type is used. Browser video controls expose playback and volume if autoplay with sound is blocked.

Source policy and permission reference: [Android playback capture](https://developer.android.com/media/platform/av-capture), [foreground service types](https://developer.android.com/develop/background-work/services/fgs/service-types#media-projection). Local fixture/build checks do not establish physical Quest audibility, synchronization, overhead or protected-app behavior.
