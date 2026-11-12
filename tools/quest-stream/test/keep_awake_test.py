import importlib.util
from pathlib import Path
import subprocess
import unittest

spec = importlib.util.spec_from_file_location('keep_awake', Path(__file__).parents[1] / 'keep-awake.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FakeAdb:
    def __init__(self):
        self.stream = False
        self.pids = '13952'
        self.error = False
        self.pid_error = False
        self.actions = []

    def __call__(self, *args):
        if self.error:
            return subprocess.CompletedProcess(args, 1, '', 'error: device offline')
        if args == ('dumpsys', 'media_projection'):
            return subprocess.CompletedProcess(args, 0, '(cz.honza.queststream, 123)' if self.stream else '', '')
        if args[0] == 'pidof' and self.pid_error:
            return subprocess.CompletedProcess(args, 1, '', 'error: device offline')
        if args[0] == 'pidof':
            return subprocess.CompletedProcess(args, 0 if self.pids else 1, self.pids, '')
        self.actions.append(args[-1].rsplit('.', 1)[-1])
        return subprocess.CompletedProcess(args, 0, 'Broadcast completed: result=0', '')


class KeepAwakeTests(unittest.TestCase):
    def test_follow_keeps_override_across_restart_then_restores_after_grace(self):
        adb = FakeAdb()
        now = [0.]
        app = module.AppLease('cz.kristek.neon.nativevr', follow=True,
                              clock=lambda: now[0], report=lambda _: None)
        watcher = module.KeepAwake(adb, app, lambda _: None)
        watcher.poll()
        adb.pids = ''
        watcher.poll()
        now[0] = 29.
        watcher.poll()
        adb.pids = '14000'
        watcher.poll()
        self.assertEqual(app.pid, 14000)
        self.assertEqual(adb.actions, ['prox_close'])
        adb.pids = ''
        watcher.poll()
        now[0] = 59.
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])
        adb.pids = '14001'
        watcher.poll()
        self.assertEqual(app.pid, 14001)
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable', 'prox_close'])
        watcher.close()
        self.assertEqual(adb.actions[-1], 'automation_disable')

    def test_follow_waits_for_first_launch_and_rejects_ambiguous_pids(self):
        app = module.AppLease('cz.kristek.neon.nativevr', follow=True, report=lambda _: None)
        self.assertFalse(app.observe(set()))
        self.assertFalse(app.ended)
        with self.assertRaises(RuntimeError):
            app.observe({1, 2})
        self.assertIsNone(app.pid)
        self.assertTrue(app.observe({3}))

    def test_follow_disconnect_is_not_an_app_exit(self):
        adb = FakeAdb()
        app = module.AppLease('cz.kristek.neon.nativevr', follow=True, report=lambda _: None)
        watcher = module.KeepAwake(adb, app, lambda _: None)
        watcher.poll()
        adb.pid_error = True
        with self.assertRaises(RuntimeError):
            watcher.poll()
        self.assertIsNone(app.missing_since)
        self.assertEqual(app.pid, 13952)
        adb.pid_error = False
        adb.pids = '14000'
        watcher.poll()
        self.assertEqual(app.pid, 14000)
        self.assertEqual(adb.actions, ['prox_close', 'prox_close'])

    def watcher(self, adb, pid=13952):
        return module.KeepAwake(adb, module.AppLease('cz.kristek.neon.nativevr', pid, lambda _: None), lambda _: None)

    def test_stream_only_preserves_existing_policy(self):
        adb = FakeAdb()
        watcher = module.KeepAwake(adb, report=lambda _: None)
        watcher.poll()
        adb.stream = True
        watcher.poll()
        watcher.poll()
        adb.stream = False
        watcher.poll()
        self.assertEqual(adb.actions, ['automation_disable', 'prox_close', 'automation_disable'])

    def test_union_does_not_restore_until_both_scopes_end(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        adb.stream = True
        watcher.poll()
        adb.pids = ''
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close'])
        adb.stream = False
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])

    def test_stream_stopping_does_not_disable_live_native_lease(self):
        adb = FakeAdb()
        adb.stream = True
        watcher = self.watcher(adb)
        watcher.poll()
        adb.stream = False
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close'])
        watcher.close()
        watcher.close()
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])

    def test_new_process_never_rearms_old_lease(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        adb.pids = '14000'
        watcher.poll()
        adb.pids = '13952'
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])

    def test_connection_failure_does_not_end_lease(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        adb.error = True
        with self.assertRaises(RuntimeError):
            watcher.poll()
        self.assertFalse(watcher.app.ended)
        self.assertEqual(adb.actions, ['prox_close'])
        adb.error = False
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'prox_close'])
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'prox_close'])

    def test_reconnect_restores_sensor_if_app_exited_during_outage(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        adb.error = True
        with self.assertRaises(RuntimeError):
            watcher.poll()
        adb.error = False
        adb.pids = ''
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])

    def test_timeout_invalidates_override_without_ending_app_lease(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        original = watcher.command
        def timeout(*args):
            raise subprocess.TimeoutExpired(args, 8)
        watcher.command = timeout
        with self.assertRaises(subprocess.TimeoutExpired):
            watcher.poll()
        self.assertIsNone(watcher.overridden)
        self.assertFalse(watcher.app.ended)
        watcher.command = original
        watcher.poll()
        self.assertEqual(adb.actions, ['prox_close', 'prox_close'])

    def test_close_before_first_sample_restores_adopted_override(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.close()
        watcher.close()
        self.assertEqual(adb.actions, ['automation_disable'])

    def test_pid_query_failure_is_not_process_exit(self):
        adb = FakeAdb()
        watcher = self.watcher(adb)
        watcher.poll()
        adb.pid_error = True
        with self.assertRaises(RuntimeError):
            watcher.poll()
        self.assertFalse(watcher.app.ended)
        self.assertEqual(adb.actions, ['prox_close'])

    def test_capture_pid_once_or_finish_if_app_absent(self):
        adb = FakeAdb()
        watcher = self.watcher(adb, None)
        watcher.poll()
        self.assertEqual(watcher.app.pid, 13952)
        other = self.watcher(adb, None)
        adb.pids = ''
        other.poll()
        adb.pids = '14000'
        other.poll()
        self.assertTrue(other.app.ended)
        self.assertEqual(adb.actions, ['prox_close', 'automation_disable'])


if __name__ == '__main__':
    unittest.main()
