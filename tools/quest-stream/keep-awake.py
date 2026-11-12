#!/usr/bin/env python3
"""Keep Quest awake for capture or an explicitly selected native app session."""
import argparse
import shutil
import signal
import subprocess
import threading
import time


def log(message):
    print(message, flush=True)


class AppLease:
    """One process by default; explicit follow mode tolerates 30s restarts."""
    def __init__(self, package, pid=None, report=log, *, follow=False, clock=time.monotonic):
        self.package = package
        self.pid = pid
        self.ended = False
        self.report = report
        self.follow = follow
        self.clock = clock
        self.missing_since = None

    def observe(self, pids):
        if self.follow:
            if len(pids) > 1:
                raise RuntimeError('Multiple app PIDs; cannot follow an ambiguous process')
            if pids:
                pid = next(iter(pids))
                if pid != self.pid or self.missing_since is not None:
                    self.report(f'Native app follow acquired: {self.package} PID {pid}')
                self.pid = pid
                self.missing_since = None
                return True
            if self.pid is None:
                return False
            if self.missing_since is None:
                self.missing_since = self.clock()
                self.report(f'Native app restart grace (30s): {self.package}')
            if self.clock() - self.missing_since < 30:
                return True
            self.report(f'Native app inactive: {self.package}; waiting for next launch')
            self.pid = None
            self.missing_since = None
            return False
        if self.ended:
            return False
        if self.pid is None:
            if len(pids) > 1:
                raise RuntimeError('Multiple app PIDs; select one with --app-pid')
            if pids:
                self.pid = next(iter(pids))
                self.report(f'Native app lease acquired: {self.package} PID {self.pid}')
        if self.pid is None or self.pid not in pids:
            self.ended = True
            self.report(f'Native app lease ended: {self.package} PID {self.pid}')
            return False
        return True


class KeepAwake:
    def __init__(self, command, app=None, report=log):
        self.command = command
        self.app = app
        self.report = report
        # Adopt an existing override: the first known inactive sample restores it.
        self.overridden = None

    def broadcast(self, action):
        result = self.command('am', 'broadcast', '-a',
                              f'com.oculus.vrpowermanager.{action}')
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())

    def poll(self):
        try:
            self._poll()
        except (OSError, subprocess.TimeoutExpired, RuntimeError):
            # Reconnect may follow a headset reboot, which clears the override.
            # Keep the app lease, but reassert the current policy once reachable.
            self.overridden = None
            raise

    def _poll(self):
        result = self.command('dumpsys', 'media_projection')
        if result.returncode:
            raise RuntimeError(result.stderr.strip() or result.stdout.strip())
        stream = '(cz.honza.queststream,' in result.stdout
        native = False
        if self.app:
            result = self.command('pidof', self.app.package)
            # Android pidof returns 1 with no output when the process is gone.
            # ADB failures also use 1, but include an error: never end a lease on those.
            if result.returncode and not (result.returncode == 1
                                         and not result.stdout.strip()
                                         and not result.stderr.strip()):
                raise RuntimeError(result.stderr.strip() or result.stdout.strip())
            try:
                pids = {int(value) for value in result.stdout.split()}
                if any(pid <= 0 for pid in pids):
                    raise ValueError()
            except ValueError as error:
                raise RuntimeError('Invalid pidof response') from error
            native = self.app.observe(pids)
        if stream or native:
            if self.overridden is not True:
                self.broadcast('prox_close')
                self.overridden = True
                scopes = ['Quest Stream capture'] if stream else []
                if native:
                    scopes.append(f'{self.app.package} PID {self.app.pid}')
                self.report('Keep awake: ' + ' + '.join(scopes))
        elif self.overridden is not False:
            self.broadcast('automation_disable')
            self.overridden = False
            self.report('All scoped sessions inactive: proximity sensor restored')

    def close(self):
        if self.overridden is not False:
            self.broadcast('automation_disable')
            self.overridden = False
            self.report('Helper exit: proximity sensor restored')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--adb', default=shutil.which('adb') or 'adb')
    parser.add_argument('--adb-port', default='5037')
    parser.add_argument('--serial', required=True)
    parser.add_argument('--keep-app', help='Lease this app process in addition to capture')
    parser.add_argument('--app-pid', type=int, help='Exact current PID; otherwise capture once')
    parser.add_argument('--follow-app', action='store_true',
                        help='Follow --keep-app across restarts; restore after 30s without it')
    args = parser.parse_args()
    if args.app_pid is not None and (not args.keep_app or args.app_pid <= 0):
        parser.error('--app-pid requires --keep-app and a positive PID')
    if args.follow_app and not args.keep_app:
        parser.error('--follow-app requires --keep-app')
    base = [args.adb, '-P', args.adb_port, '-s', args.serial, 'shell']
    stop = threading.Event()
    signal.signal(signal.SIGINT, lambda *_: stop.set())
    signal.signal(signal.SIGTERM, lambda *_: stop.set())

    def command(*parts):
        return subprocess.run(base + list(parts), capture_output=True, text=True, timeout=8)

    app = AppLease(args.keep_app, args.app_pid, follow=args.follow_app) if args.keep_app else None
    watcher = KeepAwake(command, app)
    try:
        while not stop.is_set():
            try:
                watcher.poll()
            except (OSError, subprocess.TimeoutExpired, RuntimeError) as error:
                print(f'Connection unavailable: {error}', flush=True)
            stop.wait(2)
    finally:
        try:
            watcher.close()
        except Exception as error:
            print(f'Restore failed; reconnect and run automation_disable: {error}', flush=True)


if __name__ == '__main__':
    main()
