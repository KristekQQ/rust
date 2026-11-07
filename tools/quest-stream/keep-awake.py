#!/usr/bin/env python3
"""Keep Quest awake only while this app owns an active projection; restore on exit."""
import argparse
import shutil
import signal
import subprocess
import threading

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', default=shutil.which('adb') or 'adb')
    parser.add_argument('--adb-port', default='5037')
    parser.add_argument('--serial', required=True)
    args = parser.parse_args()
    base = [args.adb, '-P', args.adb_port, '-s', args.serial, 'shell']
    stop = threading.Event()
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    signal.signal(signal.SIGTERM, lambda *_: stop.set())
    # Also adopt/restore an override enabled manually before this watcher started.
    overridden = None
    def command(*parts):
        return subprocess.run(base + list(parts), capture_output=True, text=True, timeout=8)
    def restore():
        result = command('am', 'broadcast', '-a', 'com.oculus.vrpowermanager.automation_disable')
        if result.returncode: raise RuntimeError(result.stderr.strip())
    try:
        while not stop.is_set():
            try:
                result = command('dumpsys', 'media_projection')
                if result.returncode: raise RuntimeError(result.stderr.strip())
                active = '(cz.honza.queststream,' in result.stdout
                if active and overridden is not True:
                    result = command('am', 'broadcast', '-a', 'com.oculus.vrpowermanager.prox_close')
                    if result.returncode: raise RuntimeError(result.stderr.strip())
                    overridden = True
                    print('Quest Stream active: keep awake', flush=True)
                elif not active and overridden is not False:
                    restore()
                    overridden = False
                    print('Quest Stream inactive: proximity sensor restored', flush=True)
            except (OSError, subprocess.TimeoutExpired, RuntimeError) as error:
                print(f'Connection unavailable: {error}', flush=True)
            stop.wait(2)
    finally:
        if overridden:
            try: restore()
            except Exception as error: print(f'Restore failed; reconnect and run automation_disable: {error}', flush=True)

if __name__ == '__main__':
    main()
