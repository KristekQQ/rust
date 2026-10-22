#!/usr/bin/env python3
"""Dependency-free arena MCP/HTTP contract tests."""
from concurrent.futures import ThreadPoolExecutor
from functools import partial
from http.server import ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import threading
import time
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('arena_tools', ROOT / 'scripts/arena-tools.py')
arena = importlib.util.module_from_spec(spec)
spec.loader.exec_module(arena)


class BridgeTests(unittest.TestCase):
    def test_bank_schema_requires_and_validates_hand(self):
        tool = next(t for t in arena.TOOLS if t['name'] == 'arena_bank')
        self.assertEqual(tool['inputSchema']['required'], ['hand'])
        for args in [{}, {'hand': 'third'}, {'hand': 'left', 'score': 12}]:
            with self.assertRaises(ValueError): arena.validate('arena_bank', args)
        arena.validate('arena_bank', {'hand': 'right'})

    def setUp(self):
        handler = arena.handler_class()
        handler.log_message = lambda *args: None
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), partial(handler, directory=str(ROOT)))
        self.server.bridge = arena.Bridge(timeout=.25, session_ttl=.8)
        self.server.token = 'test-secret'
        self.url = 'http://127.0.0.1:' + str(self.server.server_port)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.pool = ThreadPoolExecutor(max_workers=2)

    def tearDown(self):
        self.pool.shutdown()
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()

    def request(self, path, data=None, token='test-secret', origin=None):
        headers = {'Authorization': 'Bearer ' + token}
        if origin:
            headers['Origin'] = origin
        request = Request(self.url + '/__arena_tools/' + path,
                          data=None if data is None else json.dumps(data).encode(), headers=headers)
        try:
            with urlopen(request, timeout=2) as response:
                return response.status, json.load(response)
        except HTTPError as error:
            with error:
                return error.code, json.load(error)

    def command(self, **extra):
        return self.request('call', dict(name='arena_state', arguments={}, **extra))

    def poll_command(self, timeout=.2):
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            commands = self.request('poll?session=page')[1]['commands']
            if commands:
                return commands[0]
            time.sleep(.005)
        self.fail('Command was not queued')

    def test_auth_origin_and_size(self):
        self.assertEqual(self.request('poll?session=page', token='bad')[0], 401)
        self.assertEqual(self.request('call', {}, origin='https://evil.example')[0], 403)
        self.assertEqual(self.request('call', {}, origin=self.url)[0], 400)
        self.assertEqual(self.request('call', {'large': 'x' * arena.MAX_BODY})[0], 413)

    def test_delivery_result_and_duplicate(self):
        self.request('poll?session=page')
        future = self.pool.submit(self.command)
        command = self.poll_command()
        self.assertEqual(command['name'], 'arena_state')
        self.assertIs(type(command['expiresAt']), int)
        remaining_ms = command['expiresAt'] - time.time() * 1000
        self.assertGreater(remaining_ms, 0)
        self.assertLessEqual(remaining_ms, self.server.bridge.timeout * 1000)
        self.assertEqual(self.request('poll?session=page')[1], {'commands': []})
        payload = dict(session='page', id=command['id'], result={'live': True})
        self.assertEqual(self.request('result', dict(payload, session='wrong'))[0], 400)
        self.assertEqual(self.request('result', payload)[0], 200)
        self.assertEqual(future.result(), (200, {'result': {'live': True}}))
        self.assertEqual(self.request('result', payload)[0], 400)

    def test_live_failure_preserved(self):
        self.request('poll?session=page')
        future = self.pool.submit(self.command)
        command = self.poll_command()
        self.request('result', dict(session='page', id=command['id'], error='No live VR session'))
        self.assertEqual(future.result(), (200, {'error': 'No live VR session'}))

    def test_missing_ambiguous_and_stale_sessions(self):
        self.assertIn('No connected', self.command()[1]['error'])
        self.request('poll?session=page')
        self.request('poll?session=other')
        self.assertIn('Multiple active', self.command()[1]['error'])
        future = self.pool.submit(self.command, session='page')
        command = self.poll_command()
        self.request('result', dict(session='page', id=command['id'], result={}))
        self.assertEqual(future.result()[0], 200)
        with self.server.bridge.condition:
            self.server.bridge.sessions['page'] = time.monotonic() - 2
        self.assertIn('stale', self.command(session='page')[1]['error'])

    def test_queued_timeout_cannot_execute_later(self):
        self.request('poll?session=page')
        self.assertIn('timed out', self.command()[1]['error'])
        self.assertEqual(self.request('poll?session=page')[1]['commands'], [])
        self.assertEqual(self.server.bridge.pending, {})

    def test_delivered_timeout_cannot_complete_later(self):
        self.request('poll?session=page')
        future = self.pool.submit(self.command)
        command = self.poll_command()
        self.assertIn('timed out', future.result()[1]['error'])
        self.assertEqual(self.request('result', dict(session='page', id=command['id'], result={}))[0], 400)

    def test_invalid_configuration_rejected_before_delivery(self):
        self.request('poll?session=page')
        for arguments in ({'count': True}, {'count': 11}, {'intervalMs': 10}, {'fruit': 'apple'}, {'unknown': 1}):
            self.assertEqual(self.request('call', dict(name='arena_configure', arguments=arguments))[0], 400)
        self.assertEqual(self.request('poll?session=page')[1]['commands'], [])

    def test_preset_tools_and_numeric_configuration_contract(self):
        tools = {tool['name']: tool for tool in arena.TOOLS}
        self.assertTrue({'arena_save', 'arena_load', 'arena_reset', 'arena_export', 'arena_import'} <= tools.keys())
        self.assertEqual(tools['arena_import']['inputSchema']['properties']['json']['type'], 'string')
        for name in ('arena_save', 'arena_load', 'arena_reset', 'arena_export'):
            arena.validate(name, {})
        arena.validate('arena_import', {'json': '{"count":5}'})
        with self.assertRaises(ValueError):
            arena.validate('arena_import', {'json': 'x' * (24 * 1024 + 1)})
        for field, rule in arena.CONFIG.items():
            if rule['type'] == 'number':
                arena.validate('arena_configure', {field: rule['minimum']})
                arena.validate('arena_configure', {field: rule['maximum']})
                arena.validate('arena_configure', {field: (rule['minimum'] + rule['maximum']) / 2})
                for value in (True, float('nan'), float('inf'), float('-inf'), rule['minimum'] - 0.001,
                              rule['maximum'] + 0.001):
                    with self.assertRaises(ValueError, msg=f'{field} accepted {value}'):
                        arena.validate('arena_configure', {field: value})

    def test_real_stdio_mcp_call_via_http(self):
        self.request('poll?session=page')
        messages = [dict(jsonrpc='2.0', id=1, method='initialize', params={'protocolVersion': '2024-11-05'}),
                    dict(jsonrpc='2.0', method='notifications/initialized'),
                    dict(jsonrpc='2.0', id=2, method='tools/list'),
                    dict(jsonrpc='2.0', id=3, method='tools/call', params={'name': 'arena_state', 'arguments': {}})]
        process = subprocess.Popen([sys.executable, str(ROOT / 'scripts/arena-tools.py'), '--mcp', '--url', self.url,
                                    '--token', 'test-secret'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        future = self.pool.submit(process.communicate, '\n'.join(json.dumps(message) for message in messages) + '\n', timeout=3)
        command = self.poll_command(timeout=2)
        self.request('result', dict(session='page', id=command['id'], result={'live': True, 'count': 5}))
        stdout, stderr = future.result()
        self.assertEqual(process.returncode, 0, stderr)
        self.assertEqual(stderr, '')
        responses = [json.loads(line) for line in stdout.splitlines()]
        self.assertEqual([response['id'] for response in responses], [1, 2, 3])
        self.assertEqual(responses[0]['result']['capabilities'], {'tools': {}})
        self.assertEqual(len(responses[1]['result']['tools']), len(arena.TOOLS))
        self.assertFalse(responses[2]['result']['isError'])
        self.assertEqual(json.loads(responses[2]['result']['content'][0]['text']), {'live': True, 'count': 5})


class RpcTests(unittest.TestCase):
    def test_call_errors_and_notifications(self):
        response = arena.rpc(dict(jsonrpc='2.0', id=1, method='tools/call', params={'name': 'arena_start'}),
                             lambda *_: {'error': 'No live VR session'})
        self.assertTrue(response['result']['isError'])
        self.assertIsNone(arena.rpc(dict(jsonrpc='2.0', method='notifications/initialized'), lambda *_: None))
        self.assertEqual(arena.rpc(dict(jsonrpc='2.0', id=2, method='ping'), None)['result'], {})
        self.assertEqual(arena.rpc(dict(jsonrpc='2.0', id=3, method='unknown'), None)['error']['code'], -32601)


if __name__ == '__main__':
    unittest.main()
