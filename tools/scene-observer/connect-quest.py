#!/usr/bin/env python3
"""Install the prepared observer APK and connect a selected physical Quest.

Preserves the existing APK/config/scores in a local evidence directory. Does not
change Quest Stream, power settings, ADB servers, or tracking/guardian settings.
"""
import argparse
import hashlib
import ipaddress
import json
from pathlib import Path
import subprocess
import time
import urllib.request

PACKAGE = 'cz.kristek.neon.nativevr'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', default='/Users/Honza/Library/Android/sdk/platform-tools/adb')
    parser.add_argument('--adb-port', default=5038, type=int)
    parser.add_argument('--serial', required=True)
    parser.add_argument('--host', required=True, help='Mac Wi-Fi IPv4 address reachable from Quest')
    parser.add_argument('--server', default='http://127.0.0.1:8791')
    parser.add_argument('--apk', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    host = ipaddress.IPv4Address(args.host)
    if host.is_loopback or host.is_unspecified or host.is_multicast:
        parser.error('--host must be the Mac LAN address')
    endpoint = args.server.rstrip('/') + '/api/native'
    with urllib.request.urlopen(endpoint, timeout=3) as response:
        bridge = json.load(response)
    if not bridge.get('token') or not bridge.get('port'):
        raise RuntimeError('Native pose receiver is not running')

    def adb(*command, data=None, check=True):
        result = subprocess.run([args.adb, '-P', str(args.adb_port), '-s', args.serial, *command],
                                input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120)
        if check and result.returncode:
            raise RuntimeError(result.stderr.decode(errors='replace').strip() or result.stdout.decode(errors='replace').strip())
        return result

    def shell(command, **kwargs):
        return adb('shell', command, **kwargs)

    if adb('get-state').stdout.strip() != b'device':
        raise RuntimeError('Selected Quest is not connected')
    model = shell('getprop ro.product.model').stdout.decode().strip()
    if 'quest' not in model.lower():
        raise RuntimeError('Selected device is not a Quest: ' + model)
    # A diagnostic replay would provide scripted rather than physical tracking.
    replay = shell(f'run-as {PACKAGE} test -f files/neon-replay.json', check=False)
    if replay.returncode == 0:
        raise RuntimeError('Diagnostic replay config is present; preserve it and resolve the active measurement first')
    args.out.mkdir(parents=True, exist_ok=False)
    old_path = shell(f'pm path {PACKAGE}').stdout.decode().strip().splitlines()
    old_path = next((line[8:] for line in old_path if line.startswith('package:')), None)
    if old_path:
        adb('pull', old_path, str(args.out / 'previous.apk'))
    previous = {}
    for name in ['neon-observer.json', 'neon-top5.json', 'neon-settings.json']:
        # exec-out may report success while returning a remote cat error as data.
        # Check existence through shell first and validate before storing a backup.
        exists = shell(f'run-as {PACKAGE} test -f files/{name}', check=False)
        if exists.returncode == 0:
            result = adb('exec-out', 'run-as', PACKAGE, 'cat', 'files/' + name)
            json.loads(result.stdout)
            (args.out / ('previous-' + name)).write_bytes(result.stdout)
            previous[name] = hashlib.sha256(result.stdout).hexdigest()
    apk_sha = hashlib.sha256(args.apk.read_bytes()).hexdigest()
    adb('install', '-r', str(args.apk))
    path = shell(f'pm path {PACKAGE}').stdout.decode().strip().splitlines()[0][8:]
    installed = shell('sha256sum ' + path).stdout.decode().split()[0]
    if installed != apk_sha:
        raise RuntimeError('Installed APK identity mismatch')
    config = {'host': str(host), 'port': bridge['port'], 'token': bridge['token']}
    (args.out / 'neon-observer.json').write_text(json.dumps(config) + '\n')
    shell(f"run-as {PACKAGE} sh -c 'cat > files/neon-observer.json'", data=json.dumps(config).encode())
    shell(f'am force-stop {PACKAGE}')
    shell(f'am start -n {PACKAGE}/android.app.NativeActivity')
    deadline = time.monotonic() + 20
    status = {}
    while time.monotonic() < deadline:
        with urllib.request.urlopen(endpoint, timeout=3) as response:
            status = json.load(response)
        if status.get('connected') and int(status.get('sequence', 0)) > 10:
            break
        time.sleep(.25)
    status.pop('token', None)
    evidence = {'model': model, 'serial': args.serial, 'adbPort': args.adb_port,
                'apkSha256': apk_sha, 'previousFilesSha256': previous, 'receiver': status,
                'physicalTrackingAcceptance': 'pending user movement check', 'fpsImpact': 'not measured'}
    (args.out / 'CONNECTION.json').write_text(json.dumps(evidence, indent=2) + '\n')
    print(json.dumps(evidence, indent=2))
    if not status.get('connected'):
        print('APK is installed; put on the headset to activate OpenXR tracking.')


if __name__ == '__main__':
    main()
