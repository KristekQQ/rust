#!/usr/bin/env python3
"""Paired frozen-stereo GPU diagnostics on an explicitly selected physical Quest 3.
No APK install, process kill, clock lock, network discovery or service mutation.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shlex
import statistics
import struct
import subprocess
import sys
import time
import uuid

EFFECTS = dict(ssrStrength=0.92, ssrScale=0.5, glowStrength=0.4, glowRadius=0.045)


def sha(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def validate_elf(path):
    with Path(path).open('rb') as stream:
        head = stream.read(64)
    if len(head) != 64 or head[:6] != b'\x7fELF\x02\x01' or struct.unpack_from('<H', head, 18)[0] != 183:
        raise ValueError('binary must be a little-endian ELF64 AArch64 executable')
    if struct.unpack_from('<H', head, 16)[0] not in (2, 3):
        raise ValueError('ELF must be executable or position-independent executable')


def paired_order(pairs):
    return [('baseline', 'candidate') if i % 2 == 0 else ('candidate', 'baseline') for i in range(pairs)]


def distribution(values):
    values = sorted(values)
    if not values:
        raise ValueError('no timing samples')
    median = statistics.median(values)
    return dict(n=len(values), medianMs=median, minMs=values[0], maxMs=values[-1],
                p95Ms=values[max(0, math.ceil(len(values) * .95) - 1)],
                madMs=statistics.median(abs(v - median) for v in values))


def validate_benchmark(data, variant, expected):
    half_glow = expected.get('experiment', 'font-atlas') == 'half-glow'
    if (data.get('executionOS') != 'android' or data.get('physicalQuest') is not True
            or data.get('deviceLabel') != 'Quest_3' or data.get('openxrRuntime') is not False
            or data.get('fpsMeasurement') is not False):
        raise ValueError('not a physical Android Quest 3 frozen offscreen diagnostic')
    adapter = data.get('adapter', {})
    if adapter.get('backend') != 'Vulkan' or 'adreno' not in adapter.get('name', '').lower():
        raise ValueError('expected actual Vulkan Adreno adapter')
    if 'TIMESTAMP_QUERY' not in adapter.get('requestedFeatures', ''):
        raise ValueError('GPU timestamp feature not enabled')
    config = data.get('config', {})
    checks = dict(frames=expected['frames'], warmupFrames=expected['warmup'],
                  totalRenderedFrames=expected['frames'] + expected['warmup'],
                  width=expected['size'], height=expected['size'], periodMs=expected['period_ms'],
                  eyesPerFrame=2, ssrEnabled=True, glowOff=False, frozenSimulation=True,
                  fontGlowAtlas=half_glow or variant == 'candidate', format='Rgba8UnormSrgb', depthStore='Discard')
    if half_glow:
        checks.update(glowResolution=0.5 if variant == 'candidate' else 1.0,
                      view=expected.get('view', 'player'))
        if expected.get('view') == 'cut':
            arguments = config.get('arguments', [])
            try:
                actual_age = float(arguments[arguments.index('--cut-age') + 1])
            except (ValueError, IndexError, TypeError):
                raise ValueError('cut fixture age missing from actual executable arguments')
            if actual_age != expected.get('cut_age', 0.08):
                raise ValueError('cut fixture age mismatch')
    for key, value in checks.items():
        if config.get(key) != value:
            raise ValueError(f'config mismatch {key}: {config.get(key)!r} != {value!r}')
    effects = config.get('effectParams', {})
    for key, value in EFFECTS.items():
        actual = effects.get(key)
        if isinstance(actual, bool) or not isinstance(actual, (int, float)) or not math.isclose(actual, value, rel_tol=0, abs_tol=1e-6):
            raise ValueError(f'missing/mismatched actual effect parameter {key}')
    renderer = config.get('renderer', {})
    atlas = renderer.get('fontGlowAtlas')
    if half_glow:
        if not isinstance(atlas, dict):
            raise ValueError('half-glow comparison must retain the enabled font atlas')
    elif variant == 'candidate':
        if not isinstance(atlas, dict) or atlas.get('drawnInstances', 0) <= 0:
            raise ValueError('candidate did not draw any atlas glyph instances')
    elif atlas is not None:
        raise ValueError('baseline unexpectedly has an atlas renderer')
    if half_glow:
        half = renderer.get('halfGlow', {})
        scale = checks['glowResolution']
        if half.get('requestedScale') != scale or half.get('activeScale') != scale:
            raise ValueError('half-glow request/effective path mismatch or fallback')
        if variant == 'candidate':
            if half.get('draws', 0) <= 0 or half.get('width') != (expected['size'] + 1) // 2 or half.get('height') != (expected['size'] + 1) // 2:
                raise ValueError('half-glow target unused or wrong dimensions')
    totals, frames = [], []
    diagnostics = []
    for sample in data.get('samples', []):
        gpu = sample.get('gpu', {})
        frame = gpu.get('sampleFrame')
        if not isinstance(frame, int) or frame <= expected['warmup'] or frame > checks['totalRenderedFrames']:
            raise ValueError('invalid post-warmup sample frame')
        if frames and frame <= frames[-1]:
            raise ValueError('GPU samples must be unique and increasing')
        if gpu.get('supported') is not True or gpu.get('available') is not True or gpu.get('error'):
            raise ValueError('GPU timestamp sample unavailable/error')
        passes = gpu.get('passesMs')
        if not isinstance(passes, dict) or not passes:
            raise ValueError('missing whole-pass timing map')
        if any(k.startswith('detail.') for k in passes):
            raise ValueError('nested sections in whole-pass timing map')
        if any(isinstance(v, bool) or not isinstance(v, (int, float)) or not math.isfinite(v) or v < 0
               for v in passes.values()):
            raise ValueError('invalid whole-pass duration')
        total = sum(passes.values())  # Never include sectionsMs or CPU timings.
        if total <= 0:
            raise ValueError('zero GPU timing')
        frames.append(frame)
        totals.append(total)
        diagnostics.append(dict(sampleFrame=frame, ageFrames=gpu.get('ageFrames'),
                                skippedSamples=gpu.get('skippedSamples'), phase=sample.get('phase')))
    minimum_samples = min(5, max(1, expected['frames'] // 60))
    if len(totals) < minimum_samples:
        raise ValueError(f'need at least {minimum_samples} unique post-warmup GPU samples, got {len(totals)}')
    final = data.get('finalGpu', {})
    if final.get('error'):
        raise ValueError('final GPU profiler error')
    return dict(wholePass=distribution(totals), rawWholePassTotalsMs=totals,
                sampleDiagnostics=diagnostics, finalGpu=final)


def write_json(path, value):
    Path(path).write_text(json.dumps(value, indent=2) + '\n')


class Adb:
    def __init__(self, port, serial, out, executable='adb'):
        self.prefix = [executable, '-P', str(port), '-s', serial]
        self.out = out
        self.events = []

    def run(self, *args, timeout=60, required=True):
        command = self.prefix + list(args)
        start = time.time()
        event = dict(command=command, started=start)
        try:
            result = subprocess.run(command, capture_output=True, timeout=timeout)
            event.update(returncode=result.returncode,
                         stdout=result.stdout.decode('utf-8', errors='replace'),
                         stderr=result.stderr.decode('utf-8', errors='replace'))
        except (subprocess.TimeoutExpired, OSError) as error:
            event.update(error=str(error))
            if isinstance(error, subprocess.TimeoutExpired):
                event.update(stdout=(error.stdout or b'').decode('utf-8', errors='replace'),
                             stderr=(error.stderr or b'').decode('utf-8', errors='replace'))
            self.events.append(event)
            write_json(self.out / 'commands.json', self.events)
            raise RuntimeError(f'ADB command failed: {command!r}: {error}') from error
        event['elapsedSeconds'] = time.time() - start
        self.events.append(event)
        write_json(self.out / 'commands.json', self.events)
        if required and result.returncode:
            raise RuntimeError(f'ADB command failed ({result.returncode}): {command!r}: {event["stderr"]}')
        return event

    def shell(self, command, **kwargs):
        return self.run('shell', command, **kwargs)

    def text(self, command, **kwargs):
        return self.shell(command, **kwargs)['stdout'].strip()


def parse_probe_pid(value):
    value = value.strip()
    if not re.fullmatch(r'[1-9][0-9]{0,9}', value):
        raise ValueError('invalid owned probe PID')
    return int(value)


def owns_probe_cmdline(pid, cmdline, executable):
    parse_probe_pid(str(pid))
    return '\0' in cmdline and cmdline.split('\0', 1)[0] == executable


def probe_wrapper(command, assets, pidfile):
    # The pidfile is written before exec; exec keeps that same PID. If ADB
    # disconnects, cleanup can inspect the exact executable rather than guess.
    inner = 'printf "%s" "$$" > ' + shlex.quote(pidfile)
    inner += ' && exec env NEON_PHYSICAL_DEVICE=Quest_3 NEON_ASSETS=' + shlex.quote(assets) + ' ' + shlex.join(command)
    return 'sh -c ' + shlex.quote(inner)


def guarded_signal(pid, executable, signal):
    pid = parse_probe_pid(str(pid))
    if signal not in ('TERM', 'KILL'):
        raise ValueError('unsupported owned-probe signal')
    # Recheck immediately before *each* signal to avoid killing a reused PID.
    first = f"$(tr '\\000' '\\n' < /proc/{pid}/cmdline | head -n 1)"
    return f'if [ "{first}" = {shlex.quote(executable)} ]; then kill -{signal} {pid}; else exit 3; fi'


def terminate_owned_probe(adb, pidfile, executable):
    evidence = dict(success=False, pidfile=pidfile, executable=executable, signals=[])
    raw = adb.text('cat ' + shlex.quote(pidfile), required=False, timeout=10)
    try:
        pid = parse_probe_pid(raw)
    except ValueError:
        evidence['error'] = 'owned probe PID unavailable/invalid; refusing to guess'
        return evidence
    evidence['pid'] = pid
    for iteration in range(20):
        result = adb.shell(f'cat /proc/{pid}/cmdline', required=False, timeout=10)
        if result['returncode']:
            # Confirm absence rather than treating a permission/read error as exit.
            absent = adb.shell(f'test ! -d /proc/{pid}', required=False, timeout=10)
            if absent['returncode'] == 0:
                evidence.update(success=True, reason='owned process exited')
                return evidence
            evidence['error'] = 'cannot read live PID cmdline; refusing signal/resume'
            return evidence
        cmdline = result['stdout']
        evidence['lastCmdline'] = cmdline
        if not owns_probe_cmdline(pid, cmdline, executable):
            # It may still be the wrapper before exec. Do not signal any other
            # executable, even if a pidfile points at it.
            if iteration == 19:
                evidence['error'] = 'PID argv0 is not owned executable; refusing signal/resume'
                return evidence
        elif iteration in (0, 1, 10):
            signal = 'KILL' if iteration == 10 else 'TERM'
            event = adb.shell(guarded_signal(pid, executable, signal), required=False, timeout=10)
            evidence['signals'].append(dict(signal=signal, result=event))
        time.sleep(.25)
    evidence['error'] = 'owned process exit not confirmed after bounded termination'
    return evidence


def metadata(adb, label, args):
    commands = dict(model='getprop ro.product.model', battery='dumpsys battery',
                    mediaProjection='dumpsys media_projection',
                    thermal='dumpsys thermalservice', power='dumpsys power',
                    gpuClock='cat /sys/class/kgsl/kgsl-3d0/gpuclk',
                    gpuClockBounds='cat /sys/class/kgsl/kgsl-3d0/devfreq/min_freq /sys/class/kgsl/kgsl-3d0/devfreq/max_freq',
                    processes='ps -A -o PID,NAME')
    if args.awake_pid:
        commands['awakeHelper'] = f'cat /proc/{args.awake_pid}/status'
    result = dict(timestamp=time.time())
    for key, command in commands.items():
        result[key] = adb.shell(command, required=False, timeout=30)
    write_json(adb.out / f'{label}-metadata.json', result)
    return result


def package_state(adb, args):
    package = shlex.quote(args.package)
    paths = adb.text(f'pm path {package}')
    apks = [line.removeprefix('package:') for line in paths.splitlines() if line.startswith('package:')]
    if not apks:
        raise RuntimeError('installed package APK not found')
    apk_hashes = {p: adb.text('sha256sum ' + shlex.quote(p)).split()[0] for p in apks}
    score = shlex.quote(args.score_path)
    # run-as is read-only here; an absent score file is explicitly distinguished.
    score_command = f'if test -f {score}; then sha256sum {score}; else echo ABSENT; fi'
    score_hash = adb.text(f'run-as {package} sh -c {shlex.quote(score_command)}')
    return dict(pid=adb.text(f'pidof {package}', required=False), apkHashes=apk_hashes,
                packageDump=adb.text(f'dumpsys package {package}'), scoreHash=score_hash)


def verify_remote_hashes(adb, remote_root, assets, hashes):
    items = list(hashes.items())
    for start in range(0, len(items), 32):
        group = items[start:start + 32]
        paths = [remote_root + '/assets/' + path for path, _ in group]
        raw = adb.text('sha256sum ' + ' '.join(map(shlex.quote, paths)))
        actual = {}
        for line in raw.splitlines():
            digest, path = line.split(None, 1)
            actual[path.strip()] = digest
        for (relative, digest), remote in zip(group, paths):
            if actual.get(remote) != digest:
                raise RuntimeError(f'remote asset SHA mismatch: {relative}')


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ('adb-port', 'serial', 'binary', 'assets', 'out'):
        parser.add_argument('--' + flag, required=True, type=int if flag == 'adb-port' else str)
    parser.add_argument('--adb', default='adb', help='ADB executable path (SDK path supported)')
    parser.add_argument('--pairs', type=int, default=3)
    parser.add_argument('--frames', type=int, default=300)
    parser.add_argument('--warmup', type=int, default=60)
    parser.add_argument('--size', type=int, default=1728)
    parser.add_argument('--period-ms', type=float, default=16.667)
    parser.add_argument('--experiment', choices=('font-atlas', 'half-glow'), default='font-atlas')
    parser.add_argument('--view', choices=('player', 'cut'), default='player')
    parser.add_argument('--cut-age', type=float, default=0.08)
    parser.add_argument('--package', default='cz.kristek.neon.nativevr')
    parser.add_argument('--activity', default='android.app.NativeActivity')
    parser.add_argument('--score-path', default='files/neon-top5.json')
    parser.add_argument('--awake-pid', type=int, help='optional existing device-side helper PID; observed only, never changed')
    args = parser.parse_args(argv)
    if not 1 <= args.adb_port <= 65535 or not 1 <= args.pairs <= 6 or not 30 <= args.frames <= 3600:
        parser.error('port must be 1..65535; pairs 1..6; frames 30..3600')
    if not 0 <= args.warmup <= 600 or args.size % 64 or not 128 <= args.size <= 2048:
        parser.error('warmup 0..600; size a multiple of64 in128..2048')
    if not math.isfinite(args.period_ms) or not 0 <= args.period_ms <= 100:
        parser.error('period-ms must be finite in 0..100')
    if not math.isfinite(args.cut_age) or not 0 <= args.cut_age <= 0.64:
        parser.error('cut-age must be finite in 0..0.64 seconds')
    if args.awake_pid is not None and args.awake_pid <= 0:
        parser.error('awake-pid must be positive')
    if not re.fullmatch(r'[A-Za-z0-9_.]+', args.package) or not re.fullmatch(r'[A-Za-z0-9_.$]+', args.activity):
        parser.error('invalid package/activity')
    return args


def main(argv=None):
    args = parse_args(argv)
    binary, assets, out = map(lambda p: Path(p).resolve(), (args.binary, args.assets, args.out))
    validate_elf(binary)
    if not (assets / 'scene.json').is_file():
        raise ValueError('--assets must contain scene.json and its complete asset tree')
    files = sorted(p for p in assets.rglob('*') if p.is_file())
    if any(p.is_symlink() for p in assets.rglob('*')):
        raise ValueError('asset symlinks are not allowed')
    hashes = {p.relative_to(assets).as_posix(): sha(p) for p in files}
    out.mkdir(parents=True, exist_ok=False)
    remote = '/data/local/tmp/neon-hw-probe-' + uuid.uuid4().hex
    expected = dict(frames=args.frames, warmup=args.warmup, size=args.size, period_ms=args.period_ms,
                    experiment=args.experiment, view=args.view, cut_age=args.cut_age)
    manifest = dict(status='PREPARED', arguments=vars(args), expected=expected, binarySha256=sha(binary),
                    assetsSha256=hashes, sceneSha256=hashes['scene.json'], remoteRoot=remote,
                    clockLock=False, physicalQuestDiagnostic=True, order=paired_order(args.pairs),
                    scope='frozen offscreen stereo GPU pass timings; no XR compositor/FPS measurement', runs=[])
    write_json(out / 'manifest.json', manifest)  # Before any device mutation.
    adb = Adb(args.adb_port, args.serial, out, args.adb)
    initial = None
    device_validated = False
    mutation_attempted = False
    active_pidfile = None
    try:
        if adb.text('getprop ro.product.model').replace(' ', '_') != 'Quest_3':
            raise RuntimeError('explicit selected device is not Quest 3')
        device_validated = True
        initial = package_state(adb, args)
        manifest['initialPackage'] = initial
        write_json(out / 'manifest.json', manifest)
        metadata(adb, 'initial', args)
        mutation_attempted = True
        adb.shell('input keyevent HOME')
        adb.shell(f'mkdir {shlex.quote(remote)} && mkdir {shlex.quote(remote + "/assets")}')
        adb.run('push', str(binary), remote + '/neon-preview', timeout=120)
        adb.run('push', str(assets) + '/.', remote + '/assets', timeout=300)
        if adb.text('sha256sum ' + shlex.quote(remote + '/neon-preview')).split()[0] != manifest['binarySha256']:
            raise RuntimeError('remote binary SHA mismatch')
        verify_remote_hashes(adb, remote, assets, hashes)
        adb.shell('chmod 700 ' + shlex.quote(remote + '/neon-preview'))
        for pair, order in enumerate(paired_order(args.pairs)):
            for variant in order:
                name = f'pair-{pair + 1}-{variant}'
                remote_out = remote + '/' + name
                before = metadata(adb, name + '-before', args)
                if not re.search(r'(?:mWakefulness|Wakefulness)\s*[=:]\s*Awake', before['power']['stdout']):
                    raise RuntimeError('Quest is not verifiably Awake; keep-awake helper must be managed externally')
                command = [remote + '/neon-preview', '--output', remote_out, '--benchmark-frames', str(args.frames),
                           '--benchmark-warmup', str(args.warmup), '--benchmark-period-ms', str(args.period_ms),
                           '--size', str(args.size)]
                if args.view == 'cut':
                    command.extend(['--look-at-cut', '--cut-age', str(args.cut_age)])
                if args.experiment == 'half-glow':
                    command.extend(['--glow-resolution', '0.5' if variant == 'candidate' else '1'])
                elif variant == 'baseline':
                    command.append('--no-font-glow-atlas')
                active_pidfile = remote + '/' + name + '.pid'
                shell = probe_wrapper(command, remote + '/assets', active_pidfile)
                log = adb.shell(shell, timeout=max(180, int((args.frames + args.warmup) * args.period_ms / 1000) + 180), required=False)
                # A successful remote wait proves the owned child has exited.
                if log['returncode'] == 0:
                    active_pidfile = None
                (out / f'{name}.stdout.log').write_text(log['stdout'])
                (out / f'{name}.stderr.log').write_text(log['stderr'])
                metadata(adb, name + '-after', args)
                if log['returncode']:
                    raise RuntimeError(f'{name} executable exited {log["returncode"]}; see {name}.stderr.log and .stdout.log')
                local_out = out / name
                local_out.mkdir()
                for filename in ('benchmark.json', 'report.json', 'left.png', 'right.png'):
                    adb.run('pull', remote_out + '/' + filename, str(local_out / filename), timeout=120)
                benchmark = json.loads((local_out / 'benchmark.json').read_text())
                if benchmark.get('config', {}).get('assets') != remote + '/assets':
                    raise RuntimeError('benchmark used unexpected asset root')
                for eye in ('left', 'right'):
                    with (local_out / (eye + '.png')).open('rb') as stream:
                        header = stream.read(24)
                    if len(header) != 24 or header[:8] != b'\x89PNG\r\n\x1a\n' or struct.unpack_from('>II', header, 16) != (args.size, args.size):
                        raise RuntimeError('invalid stereo PNG dimensions')
                report = json.loads((local_out / 'report.json').read_text())
                if report.get('status') != 'PASS':
                    raise RuntimeError(f'{name} image QA failed')
                timing = validate_benchmark(benchmark, variant, expected)
                manifest['runs'].append(dict(pair=pair + 1, variant=variant, path=name, **timing))
                write_json(out / 'manifest.json', manifest)
        paired = []
        for pair in range(1, args.pairs + 1):
            values = {r['variant']: r['wholePass']['medianMs'] for r in manifest['runs'] if r['pair'] == pair}
            paired.append(dict(pair=pair, baselineMs=values['baseline'], candidateMs=values['candidate'],
                               changePercent=100 * (values['candidate'] / values['baseline'] - 1)))
        changes = [p['changePercent'] for p in paired]
        write_json(out / 'summary.json', dict(pairs=paired, medianChangePercent=statistics.median(changes),
                   minChangePercent=min(changes), maxChangePercent=max(changes),
                   clockLock=False, fpsMeasurement=False, wholePassOnly=True,
                   warning='Measured unlocked-clock offscreen GPU timings, not headset FPS or compositor performance.'))
        manifest['status'] = 'PASS'
    except Exception as caught:
        manifest.update(status='FAILED', error=str(caught))
    finally:
        restoration = dict(success=False, attempted=False)
        cleanup = dict(success=True, reason='no potentially running owned probe')
        if active_pidfile is not None:
            try:
                cleanup = terminate_owned_probe(adb, active_pidfile, remote + '/neon-preview')
            except Exception as caught:
                cleanup = dict(success=False, error=str(caught))
        manifest['probeCleanup'] = cleanup
        try:
            if not mutation_attempted:
                raise RuntimeError('restoration skipped: preflight failed before HOME/device mutation')
            if not cleanup['success']:
                raise RuntimeError('restoration skipped: owned probe termination could not be verified')
            if not device_validated:
                raise RuntimeError('restoration skipped: selected device was not verified as Quest 3; no mutation authorized')
            restoration['attempted'] = True
            launch = adb.shell('am start -W -n ' + shlex.quote(args.package + '/' + args.activity), required=False, timeout=60)
            final = package_state(adb, args)
            foreground = adb.text('dumpsys activity activities', required=False)
            (out / 'restoration-activities.log').write_text(foreground)
            foreground_confirmed = bool(re.search(r'(?:mResumedActivity|topResumedActivity|ResumedActivity)[^\n]*' + re.escape(args.package) + r'/', foreground))
            metadata(adb, 'final', args)
            restored = launch['returncode'] == 0 and bool(final['pid']) and foreground_confirmed
            restoration.update(launch=launch, actualPackage=final, foregroundConfirmed=foreground_confirmed,
                visualAcceptance=False, processRetained=initial is not None and final['pid'] == initial['pid'],
                packageUnchanged=initial is not None and final['apkHashes'] == initial['apkHashes'],
                scoreUnchanged=initial is not None and final['scoreHash'] == initial['scoreHash'],
                priorPid=None if initial is None else initial['pid'], actualPid=final['pid'])
            restoration['success'] = restored and restoration['packageUnchanged'] and restoration['scoreUnchanged']
        except Exception as caught:
            restoration['error'] = str(caught)
        manifest['restoration'] = restoration
        if not restoration['success']:
            manifest['status'] = 'FAILED'
        write_json(out / 'manifest.json', manifest)
    print(json.dumps(dict(status=manifest['status'], output=str(out), restoration=restoration['success'])))
    return 0 if manifest['status'] == 'PASS' else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
