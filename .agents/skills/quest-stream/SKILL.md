---
name: quest-stream
description: Run, diagnose or extend Quest Stream, this project's standalone Meta Quest screen broadcaster to a local browser over Wi-Fi. Use for headset mirroring and off-face streaming, not WebXR rendering backend support.
---

# Quest Stream

The application is **Quest Stream**, Android package `cz.honza.queststream`, activity `.MainActivity`. Its maintained home is [tools/quest-stream](../../../tools/quest-stream/README.md). Use this integrated copy rather than older `output/worktrees/quest-stream-*` snapshots. It captures Horizon OS MediaProjection into a hardware H.264 encoder and serves fragmented MP4 to a MediaSource browser viewer; it does not render the game on the laptop or modify the engine.

## Start a session

- Reuse the installed working APK and current capture permission. Reinstall only when needed: updating the APK stops capture and requires fresh in-headset consent. Build/install details and existing signing-key handling are in the tool README. Preserve `build/development.keystore`; never commit the key or session URLs/tokens.
- Discover ADB and `adb devices -l`. On macOS the SDK's `platform-tools/adb` may be outside PATH. Derive the actual headset IPv4 address with `adb -s USB_SERIAL shell ip -f inet addr show wlan0`; do not assume an old IP or USB serial is current.
- If Wi-Fi ADB is not already active, use the authorized USB connection for `adb -s USB_SERIAL tcpip 5555`, then `adb connect QUEST_IP:5555`. Verify using `adb -s QUEST_IP:5555 shell getprop ro.product.model` before telling Honza to unplug. All further operations must target the Wi-Fi serial explicitly.
- If a running ADB server reports `No route to host` but a host TCP probe reaches port 5555, try an unused isolated server port, e.g. `adb -P 5038 connect QUEST_IP:5555`. Carry `-P PORT` consistently into every following command and helper; do not kill unrelated ADB servers or their forwards.
- Launch with `adb -P PORT -s QUEST_IP:5555 shell am start -n cz.honza.queststream/.MainActivity`. Check `shell dumpsys media_projection` for this package before requesting consent again. When needed Honza chooses **Povolit přenos → Celé zobrazení → Sdílet** in the headset; never claim to have bypassed OS consent.
- Read the current URL from the app, using the actual headset screen or a freshly successful `uiautomator dump` via the Android CLI. Failed/empty dumps and old files are not current evidence. An asleep headset can have active projection but no UI tree; wake it and relaunch the activity before rereading. Do not guess or reuse another session's token.
- For the requested laptop-local address, `adb -P PORT -s QUEST_IP:5555 forward tcp:8088 tcp:8088` makes `http://localhost:8088/?token=CURRENT_TOKEN` work over **Wi-Fi ADB**, without a cable. Direct `http://QUEST_IP:8088/?token=…` also serves clients on the LAN where allowed. Preserve other tasks' listeners; inspect occupied ports rather than replacing them.
- Use the enabled unified browser tools. Prefer a visible receiver beside the chat when supported, otherwise explicitly show Chrome. Click **Sledovat**, verify actual decoded video, and keep the working tab marked as the deliverable. Opening an HTTP page or seeing an encoder status alone does not prove playback. Avoid opening extra receiver tabs as routine handoff.

## Keep streaming when removed

Honza wants the headset to remain active when removed during streaming. Run the existing `tools/quest-stream/keep-awake.py --adb /path/to/adb --adb-port PORT --serial QUEST_IP:5555` helper in a retained process, reusing a matching running helper instead of launching duplicates. Record its PID/log location in local task coordination, not source documentation. It polls capture ownership, applies Meta's `prox_close` while Quest Stream owns capture and restores `automation_disable` when capture stops or the helper exits. An ADB outage can delay restoration; see the tool README for recovery. Do not disable unrelated headset safety features or set an unrelated permanent sleep policy.

## Diagnose before restarting

- The current implementation supports **two simultaneous browser readers** with independent bounded queues. Reloading or disconnecting a browser does not consume a new projection token or stop the capture session. A third reader should get an explicit two-window message; close an unnecessary reader and retry **Sledovat**, without immediately asking for new consent.
- The first prototype used single-use reader claims and the ambiguous message “Přenos už má diváka nebo skončil”. If that UI reappears, check installed APK/source identity and cached/old viewer tabs; it is not proof that Wi-Fi failed or permission expired.
- Diagnose Wi-Fi ADB connectivity, `dumpsys media_projection`, `dumpsys power` (`mWakefulness`), authenticated `/status?token=…`, helper logs and browser video state separately. Reuse a live permission/session when possible. A genuine system stop or encoder failure needs fresh consent; a browser disconnect does not.
- For a phone or another computer, give the current **headset LAN address**, not the laptop's localhost ADB forward. Distinguish a page that will not load from failure after **Sledovat**. If host HTTP succeeds but macOS Chrome reports `ERR_ADDRESS_UNREACHABLE`, inspect its **Privacy & Security → Local Network** permission; a disabled permission was observed in this setup. This does not diagnose a separate phone. Follow the applicable permission rules before changing it; a browser restart may be needed and can interrupt other open work.
- Capture starts promptly after consent to avoid delayed token expiration. It continues until stopped in the headset, including when there are no viewers. Stop sharing after use; a browser tab closing alone is intentionally insufficient.

## Change and verify

Keep this as a standalone tool under `tools/quest-stream/`; do not add capture rendering passes to Rust/WASM or rebuild the engine for companion-only changes. Follow the repository's parallel-work rules for independent changes and integration.

Run `tools/quest-stream/test/verify-local.sh` and `tools/quest-stream/build.sh` for affected Java/media changes. Reuse unchanged passing evidence when only copying verified source or editing documentation. Test actual Quest playback, same-session reload, late second reader and first-reader continuity after closing the second. Check off-face behavior and sensor restoration when changing the awake helper.

Evidence as of 2026-11-07: physical Quest 3 → local browser at 1280×720 over Wi-Fi, same-token reload and two simultaneous decoded viewers passed; the keep-awake helper's active/inactive transitions and Awake state were observed. That earlier session was video-only; playback audio was added below. Controlled native-game/WebXR-game streaming, latency, frame pacing, battery cost and long-run/doff acceptance remain separate checks. Do not claim every Windows/macOS/Linux browser was physically tested.


## Playback audio (2026-11-12)

Current Quest Stream supports optional permitted-app playback capture → stereo
AAC, in the same fragmented MP4 stream as video. It never selects a microphone
source. Android nevertheless requires RECORD_AUDIO (shown as microphone/audio
permission); explain this distinction. A denied permission leaves video-only
capture, with explicit app/viewer status. Repeated denial can set USER_FIXED;
at the user's request clear only this app's RECORD_AUDIO user-set/user-fixed
flags, then let the user confirm the normal prompt. Never grant it silently.
Read the fresh app URL after restarting capture. Verify H.264 + AAC status and
real decoded media separately from subjective audibility/latency. Live Quest3
capture was exercised and user accepted sound; no controlled performance claim.

For game cues, mixer levels and observer event audio use
[game-audio](../game-audio/SKILL.md). A LAN observer runs on the computer, while
Quest Stream runs on the headset: their IP addresses and ports are different.
The observer's viewer-only gateway and phone access are documented in
[AUDIO.md](../../../docs/AUDIO.md). Never commit live session links or tokens.
