#!/usr/bin/env python3
"""Bounded native Quest capture. Never changes Stream, power settings or ADB service."""
import argparse
import datetime
import hashlib
import json
import math
import os
from pathlib import Path
import re
import signal
import subprocess
import time

PACKAGE = "cz.kristek.neon.nativevr"


def distribution(values):
    values = sorted(v for v in values if v is not None and math.isfinite(v))
    if not values:
        return None
    return {
        "n": len(values), "mean": sum(values) / len(values),
        **{name: values[max(0, math.ceil(len(values) * q) - 1)]
           for name, q in [("p50", .5), ("p95", .95), ("p99", .99), ("max", 1)]},
    }


def summarize(run):
    frames = run["frames"]
    rendered = [f for f in frames if f["should_render"] and f["tracking_valid"]]
    focused = [f for f in rendered if f["focused"]]
    intervals = [f["app_interval_ns"] / 1e6 for f in rendered if f["app_interval_ns"] is not None]
    # Worker snapshots may be applied on multiple XR frames: count each once.
    workers = {f["app"]["applied_sequence"]: f["app"] for f in rendered}
    samples = list(workers.values())
    counters = {key: [f["app"][key] for f in frames]
                for key in ("cuts", "shots", "cue_index", "flying", "fragments", "batched", "dropped")}
    gaps = [(b["monotonic_ns"] - a["monotonic_ns"]) / 1e6 for a, b in zip(frames, frames[1:])]
    player = [f for f in rendered if f["app"].get("replay") and f["app"]["replay"]["active"]]
    player_intervals = [f["app_interval_ns"] / 1e6 for f in player if f["app_interval_ns"] is not None]
    player_summary = None
    if player:
        player_summary = {
            "frames": len(player), "cutting": player[0]["app"]["replay"]["cutting"],
            "replay_elapsed_range_s": [player[0]["app"]["replay"]["elapsed_s"], player[-1]["app"]["replay"]["elapsed_s"]],
            "head_first": player[0]["app"]["replay"]["head_world"],
            "app_interval_ms": distribution(player_intervals),
            "app_fps_from_intervals": 1000 * len(player_intervals) / sum(player_intervals) if player_intervals else None,
            "host_work_excluding_wait_ms": distribution([sum(f["stages_ns"][1:]) / 1e6 for f in player]),
            "cuts_max": max(f["app"]["cuts"] for f in player),
            "shots_max": max(f["app"]["shots"] for f in player),
            "fragments_max": max(f["app"]["fragments"] for f in player),
            "cue_index_max": max(f["app"]["cue_index"] for f in player),
            "scope": "scripted ordinary player view/cuts on device, not worn human acceptance",
        }
    return {
        "complete": run["complete"], "valid": run["valid"], "reason": run["reason"],
        "frames": len(frames), "rendered": len(rendered), "focused": len(focused),
        "wall_span_s": (frames[-1]["monotonic_ns"] - frames[0]["monotonic_ns"]) / 1e9 if frames else 0,
        "rendered_intervals_sum_s": sum(intervals) / 1000,
        "max_record_gap_ms": max(gaps) if gaps else None,
        "record_gaps_over_1s": sum(gap > 1000 for gap in gaps),
        "nonrenderable_or_unfocused_frames": len(frames) - len(focused),
        "valid_means": "recorder transport complete; focus/coverage/workload still require acceptance",
        "display_hz": sorted({f["display_hz"] for f in frames if f["display_hz"] is not None}),
        "app_interval_ms": distribution(intervals),
        "app_fps_from_intervals": 1000 * len(intervals) / sum(intervals) if intervals else None,
        "fps_scope": "rendered intervals only; inspect wall coverage and focus gaps before performance use",
        "intervals_over_1s": sum(v > 1000 for v in intervals),
        "cadence_over_1_5_predicted_period": sum(
            f["app_interval_ns"] > 1.5 * f["period_ns"] for f in rendered
            if f["app_interval_ns"] is not None and f["period_ns"] > 0),
        "cadence_count_is_compositor_drops": False,
        "host_stages_ms": {name: distribution([f["stages_ns"][i] / 1e6 for f in rendered])
                           for i, name in enumerate(run["stage_names"])},
        "host_work_excluding_wait_ms": distribution([sum(f["stages_ns"][1:]) / 1e6 for f in rendered]),
        "worker_snapshot_ms": {key: distribution([s[key] for s in samples]) for key in (
            "worker_ms", "worker_queue_ms", "worker_game_ms", "worker_publication_ms", "worker_retirement_ms")},
        "simulation_age_ms": distribution([f["app"]["simulation_age_ms"] for f in rendered]),
        "publication_age_ms": distribution([f["app"]["publication_age_ms"] for f in rendered]),
        "draws": distribution([f["app"]["draws"] for f in rendered]),
        "upload_bytes": distribution([f["app"]["upload_bytes"] for f in rendered]),
        "counter_ranges": {key: {"first": v[0], "last": v[-1], "min": min(v), "max": max(v)}
                           for key, v in counters.items() if v},
        "profile_snapshots": len(run["profiles"]), "gpu_scope": run["profile_kind"],
        "player_replay": player_summary,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--apk", type=Path)
    parser.add_argument("--warmup", type=float, default=0)
    parser.add_argument("--seconds", type=float, default=65)
    parser.add_argument("--condition", required=True)
    parser.add_argument("--device", default="192.168.0.234:5555")
    parser.add_argument("--adb", default="/Users/Honza/Library/Android/sdk/platform-tools/adb")
    parser.add_argument("--awake-pidfile", type=Path)
    parser.add_argument("--awake-helper", type=Path)
    parser.add_argument("--player-replay", choices=("view", "cut"))
    parser.add_argument("--replay-seconds", type=float, default=65)
    parser.add_argument("--startup-timeout", type=float, default=35)
    parser.add_argument("--resume-only", action="store_true", help="clear diagnostic flags and restart normal input/score persistence")
    args = parser.parse_args()
    if bool(args.awake_pidfile) != bool(args.awake_helper):
        parser.error("awake-pidfile and awake-helper must be supplied together")
    if args.resume_only and args.player_replay:
        parser.error("resume-only cannot enable player replay")
    if not re.fullmatch(r"[A-Za-z0-9_-]{1,64}", args.run_id):
        parser.error("invalid run-id")
    if not (math.isfinite(args.warmup) and 0 <= args.warmup <= 120
            and math.isfinite(args.seconds) and 1 <= args.seconds <= 300):
        parser.error("invalid bounded duration")
    if not (math.isfinite(args.replay_seconds) and 10 <= args.replay_seconds <= 120
            and math.isfinite(args.startup_timeout) and 1 <= args.startup_timeout <= 300):
        parser.error("invalid replay/startup duration")
    args.out.mkdir(parents=True, exist_ok=False)
    adb = [args.adb, "-P", "5038", "-s", args.device]

    def command(*parts, check=True, data=None, timeout=20):
        result = subprocess.run(adb + list(parts), input=data, capture_output=True, timeout=timeout)
        if check and result.returncode:
            raise RuntimeError(result.stderr.decode(errors="replace") + result.stdout.decode(errors="replace"))
        return result

    def shell(text, **kw):
        return command("shell", text, **kw)

    def metadata():
        queries = {
            "package": f"dumpsys package {PACKAGE} | toybox grep -E 'versionCode=|versionName='",
            "apk_sha256": f"p=$(pm path {PACKAGE}); sha256sum \"${{p#package:}}\"",
            "battery": "dumpsys battery", "thermal": "dumpsys thermalservice",
            "settings": f"run-as {PACKAGE} cat files/neon-settings.json",
            "top5_sha256": f"run-as {PACKAGE} sha256sum files/neon-top5.json",
        }
        return {name: shell(query, check=False).stdout.decode(errors="replace")
                for name, query in queries.items()}

    def renew_awake():
        """Replace only the explicitly selected helper after its restoration exits."""
        if not args.awake_pidfile:
            return
        old_pid = int(args.awake_pidfile.read_text().strip())
        expected = str(args.awake_helper.resolve())
        processes = subprocess.check_output(["ps", "-axo", "pid=,command="], text=True)
        matching = [(int(row.strip().split(None, 1)[0]), row.strip().split(None, 1)[1])
                    for row in processes.splitlines() if expected in row and "--keep-app" in row]
        if len(matching) != 1 or matching[0][0] != old_pid:
            raise RuntimeError("unexpected awake helper inventory; refusing duplicate")
        old_command = matching[0][1]
        if PACKAGE not in old_command or args.device not in old_command:
            raise RuntimeError("selected awake helper owns another session")
        deadline = time.monotonic() + 10
        while True:
            response = shell(f"pidof {PACKAGE}", check=False)
            pid = response.stdout.decode().strip()
            if response.returncode == 0 and pid.isdigit():
                break
            if time.monotonic() > deadline:
                raise RuntimeError("no unique Neon PID after startup timeout")
            time.sleep(.2)
        # Reapply the lease after OpenXR/content preparation, not during startup.
        # An early override let Android stay Awake while the new VR session paused.
        deadline = time.monotonic() + 20
        while True:
            startup = command("logcat", "-d", f"--pid={pid}", "-s", "KristekNeon:I", "*:S").stdout
            if b"OpenXR world reference:" in startup:
                break
            if time.monotonic() > deadline:
                raise RuntimeError("Neon VR startup not ready; awake replacement not started")
            time.sleep(.5)
        if "--follow-app" in old_command.split():
            # The explicit development session already follows restarted PIDs.
            # Keep its override continuously active instead of stopping it.
            log_path = args.awake_pidfile.with_suffix(".log")
            deadline = time.monotonic() + 12
            while True:
                with log_path.open("rb") as log:
                    log.seek(max(0, log_path.stat().st_size - 65536))
                    tail = log.read().decode(errors="replace")
                if f"Native app follow acquired: {PACKAGE} PID {pid}" in tail:
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError("Existing follow helper did not confirm the new Neon PID")
                time.sleep(.2)
            shell(f"am start -n {PACKAGE}/android.app.NativeActivity")
            if shell(f"pidof {PACKAGE}").stdout.decode().strip() != pid:
                raise RuntimeError("Neon changed PID while resuming")
            info["awake_handoff"] = {"previous_helper_pid": old_pid, "helper_pid": old_pid,
                                      "native_pid": int(pid), "retained_follow_helper": True}
            return
        os.kill(old_pid, signal.SIGTERM)
        deadline = time.monotonic() + 20
        while subprocess.run(["ps", "-p", str(old_pid)], capture_output=True).returncode == 0:
            if time.monotonic() > deadline:
                raise RuntimeError("old awake helper did not exit; replacement not started")
            time.sleep(.2)
        log_path = args.awake_pidfile.with_suffix(".log")
        log_offset = log_path.stat().st_size if log_path.exists() else 0
        with log_path.open("ab", buffering=0) as log:
            process = subprocess.Popen([
                "python3", "-u", expected, "--adb", args.adb, "--adb-port", "5038",
                "--serial", args.device, "--keep-app", PACKAGE, "--app-pid", pid,
            ], stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT, start_new_session=True)
        args.awake_pidfile.write_text(str(process.pid) + "\n")
        deadline = time.monotonic() + 12
        while True:
            with log_path.open("rb") as log:
                log.seek(log_offset)
                lease_log = log.read().decode(errors="replace")
            if "Keep awake:" in lease_log and f"{PACKAGE} PID {pid}" in lease_log:
                break
            if process.poll() is not None or time.monotonic() > deadline:
                raise RuntimeError("new awake helper did not confirm its native lease")
            time.sleep(.2)
        # Off-face wake may leave Horizon ClearActivity in front. Resume the same
        # process after wake; this preserves the exact PID lease and capture consent.
        shell(f"am start -n {PACKAGE}/android.app.NativeActivity")
        resumed_pid = shell(f"pidof {PACKAGE}").stdout.decode().strip()
        if resumed_pid != pid:
            raise RuntimeError("Neon changed PID on resume; awake lease needs a fresh handoff")
        info["awake_handoff"] = {"previous_helper_pid": old_pid, "helper_pid": process.pid,
                                  "native_pid": int(pid), "old_exit_confirmed": True,
                                  "resumed_after_wake": True}

    info = {"run_id": args.run_id, "condition": args.condition,
            "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "config": {"run_id": args.run_id, "warmup_s": args.warmup, "duration_s": args.seconds},
            "stream_changed": False, "power_settings_changed": False}
    previous = None
    configured = False
    previous_replay = None
    configured_replay = False
    try:
        info["before"] = metadata()
        if args.apk:
            info["installed_apk_sha256"] = hashlib.file_digest(args.apk.open("rb"), "sha256").hexdigest()
            command("install", "-r", str(args.apk.resolve()), timeout=60)
        if args.resume_only:
            shell(f"run-as {PACKAGE} rm -f files/neon-measure.json files/neon-replay.json")
            shell(f"am force-stop {PACKAGE}")
            shell(f"am start -n {PACKAGE}/android.app.NativeActivity")
            renew_awake()
            info["after"] = metadata()
            info["status"] = "normal_input_and_score_path_restored"
            print("Normal Neon input and original score path restored", flush=True)
            return
        old = shell(f"run-as {PACKAGE} cat files/neon-measure.json", check=False)
        if old.returncode == 0:
            previous = old.stdout
        elif b"No such file" not in old.stdout + old.stderr:
            raise RuntimeError("cannot determine prior measurement configuration")
        shell(f"run-as {PACKAGE} sh -c 'cat > files/neon-measure.json'",
              data=json.dumps(info["config"]).encode())
        configured = True
        if args.player_replay:
            old_replay = shell(f"run-as {PACKAGE} cat files/neon-replay.json", check=False)
            if old_replay.returncode == 0:
                previous_replay = old_replay.stdout
            elif b"No such file" not in old_replay.stdout + old_replay.stderr:
                raise RuntimeError("cannot determine prior player replay configuration")
            replay_config = {"cutting": args.player_replay == "cut", "duration_s": args.replay_seconds}
            shell(f"run-as {PACKAGE} sh -c 'cat > files/neon-replay.json'", data=json.dumps(replay_config).encode())
            configured_replay = True
            info["player_replay"] = replay_config
        shell(f"am force-stop {PACKAGE}")
        shell(f"am start -n {PACKAGE}/android.app.NativeActivity")
        renew_awake()
        print(f"Started {args.run_id}: {args.condition}", flush=True)
        destination = f"files/measurements/{args.run_id}.json"
        deadline = time.monotonic() + args.warmup + args.seconds + args.startup_timeout
        while time.monotonic() < deadline:
            ready = shell(f"run-as {PACKAGE} test -f {destination}", check=False)
            if ready.returncode == 0:
                payload = command("exec-out", "run-as", PACKAGE, "cat", destination).stdout
                (args.out / "frames.json").write_bytes(payload)
                run = json.loads(payload)
                summary = summarize(run)
                (args.out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
                print(json.dumps(summary, separators=(",", ":")), flush=True)
                info["capture_sha256"] = hashlib.sha256(payload).hexdigest()
                if not run["complete"] or not run["valid"]:
                    raise RuntimeError(f"invalid capture: {run['reason']}")
                break
            time.sleep(2)
        else:
            raise RuntimeError("capture did not complete; no focus/render or interrupted app possible")
        info["after"] = metadata()
        if args.player_replay:
            info["original_top5_unchanged"] = info["before"]["top5_sha256"] == info["after"]["top5_sha256"]
            if not info["original_top5_unchanged"]:
                raise RuntimeError("Original TOP5 changed during synthetic replay")
        info["status"] = "captured"
    except Exception as error:
        info["status"] = "failed"
        info["error"] = str(error)
        raise
    finally:
        try:
            if configured:
                if previous is None:
                    shell(f"run-as {PACKAGE} rm files/neon-measure.json")
                else:
                    shell(f"run-as {PACKAGE} sh -c 'cat > files/neon-measure.json'", data=previous)
            if configured_replay:
                if previous_replay is None:
                    shell(f"run-as {PACKAGE} rm files/neon-replay.json")
                else:
                    shell(f"run-as {PACKAGE} sh -c 'cat > files/neon-replay.json'", data=previous_replay)
            pid = shell(f"pidof {PACKAGE}", check=False).stdout.decode().strip()
            if pid.isdigit():
                logs = command("logcat", "-d", f"--pid={pid}", "-s", "KristekNeon:I", "*:S").stdout
                (args.out / "device.log").write_bytes(logs)
        except Exception as error:
            info["cleanup_error"] = str(error)
        info["finished_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (args.out / "manifest.json").write_text(json.dumps(info, indent=2) + "\n")


if __name__ == "__main__":
    main()
