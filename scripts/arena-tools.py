#!/usr/bin/env python3
"""Opt-in authenticated arena bridge and dependency-free MCP stdio client.

Bridge: python3 scripts/arena-tools.py --token SECRET --directory .
Quest: add --host 0.0.0.0 --cert cert.pem --key key.pem (use a trusted certificate).
Release: --production --directory output/production
MCP: python3 scripts/arena-tools.py --mcp --url https://HOST:8766 --token SECRET
Open the arena on that bridge origin and enable its browser bridge with the same
secret. MCP controls only a connected live page; it cannot start a VR session.
Keep the token private. No tools or network listener are enabled by default in
the ordinary development server. MCP stdout contains JSON-RPC JSONL only.
"""
import argparse
from functools import partial
import hmac
import math
from http.server import ThreadingHTTPServer
import importlib.util
import json
from pathlib import Path
import ssl
import sys
import threading
import time
from urllib.error import HTTPError, URLError
from urllib.parse import parse_qs, urlsplit
from urllib.request import Request, urlopen
import uuid

MAX_BODY = 32768
FRUITS = ['mixed', 'cherry', 'lime', 'lemon', 'orange', 'banana', 'plum', 'pear', 'grapes', 'watermelon']
CONFIG = {'count': {'type': 'integer', 'minimum': 1, 'maximum': 10},
          'intervalMs': {'type': 'integer', 'minimum': 1800, 'maximum': 6000},
          'fruitScale': {'type': 'number', 'minimum': 0.03, 'maximum': 0.105},
          'width': {'type': 'number', 'minimum': 0.45, 'maximum': 0.9},
          'height': {'type': 'number', 'minimum': 1.1, 'maximum': 1.6},
          'depth': {'type': 'number', 'minimum': -1.4, 'maximum': -0.9},
          'curve': {'type': 'number', 'minimum': 0, 'maximum': 0.25},
          'slope': {'type': 'number', 'minimum': 0, 'maximum': 0.2},
          'lift': {'type': 'number', 'minimum': 0.2, 'maximum': 0.8},
          'gravity': {'type': 'number', 'minimum': 0.5, 'maximum': 1.6},
          'lifetime': {'type': 'number', 'minimum': 1.5, 'maximum': 3.5},
          'program': {'type': 'string', 'enum': ['minute', 'practice']},
          'formation': {'type': 'string', 'enum': ['line', 'arc', 'cross', 'rain']},
          'fruit': {'type': 'string', 'enum': FRUITS}, 'automatic': {'type': 'boolean'}}
TOOLS = [{'name': name, 'description': description, 'inputSchema': {
    'type': 'object', 'properties': dict(properties, session={'type': 'string', 'description': 'Explicit active browser page session.'}),
    'additionalProperties': False}}
    for name, description, properties in [
        ('arena_state', 'Read state of the live connected arena page.', {}),
        ('arena_configure', 'Configure waves in the live connected arena page.', CONFIG),
        ('arena_preview', 'Preview an authored minute-scene cue before VR; omit seconds for the next cue.', {'seconds': {'type': 'number', 'minimum': 0, 'maximum': 59.999}}),
        ('arena_spawn', 'Spawn a wave in the live connected arena page, optionally overriding configuration.', CONFIG),
        ('arena_start', 'Start or restart a 60-second round in the live connected arena page.', {}),
        ('arena_bank', 'Bank a left/right fruit chain and release that stock in active VR.', {'hand': {'type': 'string', 'enum': ['left', 'right']}}),
        ('arena_stop', 'Disable automatic waves in the live connected arena page.', {}),
        ('arena_save', 'Save the current arena preset explicitly in browser localStorage.', {}),
        ('arena_load', 'Restore the saved arena preset from browser localStorage.', {}),
        ('arena_reset', 'Restore the repository JSON preset and clear its localStorage override.', {}),
        ('arena_export', 'Return the current repository JSON preset document.', {}),
        ('arena_import', 'Import a JSON preset document; the connected browser parses and validates it.',
         {'json': {'type': 'string', 'maxLength': 24576}})]]


for tool in TOOLS:
    if tool['name'] == 'arena_bank':
        tool['inputSchema']['required'] = ['hand']


def validate(name, arguments):
    tool = next((tool for tool in TOOLS if tool['name'] == name), None)
    if not tool or not isinstance(arguments, dict):
        raise ValueError('Unknown tool or invalid arguments')
    if name == 'arena_bank' and 'hand' not in arguments:
        raise ValueError('Missing hand')
    for key, value in arguments.items():
        rule = tool['inputSchema']['properties'].get(key)
        if rule is None:
            raise ValueError('Unknown argument: ' + key)
        kind = rule['type']
        if ((kind == 'integer' and (type(value) is not int or not rule['minimum'] <= value <= rule['maximum']))
                or (kind == 'number' and (type(value) not in (int, float)
                                          or not rule['minimum'] <= value <= rule['maximum']
                                          or not math.isfinite(value)))
                or (kind == 'boolean' and type(value) is not bool)
                or (kind == 'string' and (not isinstance(value, str) or not value
                                          or len(value.encode('utf-8')) > rule.get('maxLength', 128)))
                or ('enum' in rule and value not in rule['enum'])):
            raise ValueError('Invalid argument: ' + key)


class Bridge:
    def __init__(self, timeout=8.0, session_ttl=5.0):
        self.timeout, self.session_ttl = timeout, session_ttl
        self.condition = threading.Condition()
        self.sessions, self.pending = {}, {}

    def poll(self, session):
        if not isinstance(session, str) or not session or len(session) > 128:
            raise ValueError('Invalid session')
        with self.condition:
            now = time.monotonic()
            self.sessions = {s: seen for s, seen in self.sessions.items() if now - seen < self.session_ttl}
            if session not in self.sessions and len(self.sessions) >= 128:
                raise ValueError('Too many sessions')
            self.sessions[session] = now
            commands = []
            for command in self.pending.values():
                if command['session'] == session and not command['delivered'] and command['deadline'] > now:
                    command['delivered'] = True
                    payload = {key: command[key] for key in ('id', 'name', 'arguments')}
                    payload['expiresAt'] = int((time.time() + command['deadline'] - now) * 1000)
                    commands.append(payload)
            return {'commands': commands}

    def call(self, name, arguments, session=None):
        validate(name, arguments)
        with self.condition:
            now = time.monotonic()
            active = [s for s, seen in self.sessions.items() if now - seen < self.session_ttl]
            if session is None:
                if len(active) != 1:
                    raise ValueError('No connected arena page' if not active else 'Multiple active arena pages; specify session')
                session = active[0]
            elif session not in active:
                raise ValueError('Arena page session is missing or stale')
            if len(self.pending) >= 32:
                raise ValueError('Arena command queue is full')
            command = dict(id=uuid.uuid4().hex, name=name, arguments=arguments, session=session,
                           delivered=False, deadline=now + self.timeout)
            self.pending[command['id']] = command
            try:
                while 'response' not in command:
                    remaining = command['deadline'] - time.monotonic()
                    if remaining <= 0:
                        raise ValueError('Arena command timed out; no connected arena page response')
                    self.condition.wait(remaining)
                return command['response']
            finally:
                self.pending.pop(command['id'], None)

    def result(self, data):
        with self.condition:
            command = self.pending.get(data.get('id'))
            if (not command or command['session'] != data.get('session') or not command['delivered']
                    or command['deadline'] <= time.monotonic() or 'response' in command):
                raise ValueError('Unknown, expired, or completed command')
            if ('result' in data) == ('error' in data):
                raise ValueError('Provide exactly one of result or error')
            command['response'] = {key: data[key] for key in ('result', 'error') if key in data}
            self.condition.notify_all()
            return {'ok': True}


def handler_class(production=False):
    spec = importlib.util.spec_from_file_location('arena_static', Path(__file__).with_name('serve-local.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    base = module.ProductionHandler if production else module.DevelopmentHandler

    class Handler(base):
        def setup(self):
            super().setup()
            self.connection.settimeout(12)

        def reply(self, status, body):
            payload = json.dumps(body, ensure_ascii=False).encode()
            self.send_response(status)
            self.send_header('Content-Type', 'application/json')
            self.send_header('Content-Length', str(len(payload)))
            self.end_headers()
            self.wfile.write(payload)

        def authorized(self):
            auth = self.headers.get('Authorization', '')
            if not hmac.compare_digest(auth.encode(), ('Bearer ' + self.server.token).encode()):
                self.reply(401, {'error': 'Bearer token required'})
                return False
            origin = self.headers.get('Origin')
            expected = ('https' if isinstance(self.connection, ssl.SSLSocket) else 'http') + '://' + self.headers.get('Host', '')
            if origin and origin != expected:
                self.reply(403, {'error': 'Cross-origin requests are forbidden'})
                return False
            return True

        def do_GET(self):
            parsed = urlsplit(self.path)
            if not parsed.path.startswith('/__arena_tools/'):
                return super().do_GET()
            if not self.authorized():
                return
            try:
                if parsed.path != '/__arena_tools/poll':
                    return self.reply(404, {'error': 'Unknown endpoint'})
                session = parse_qs(parsed.query).get('session', [''])[0]
                self.reply(200, self.server.bridge.poll(session))
            except ValueError as error:
                self.reply(400, {'error': str(error)})

        def do_POST(self):
            path = urlsplit(self.path).path
            if not path.startswith('/__arena_tools/'):
                return super().do_POST()
            if not self.authorized():
                return
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if not 0 < length <= MAX_BODY or self.headers.get('Transfer-Encoding'):
                    return self.reply(413, {'error': 'Invalid body size'})
                data = json.loads(self.rfile.read(length))
                if not isinstance(data, dict):
                    raise ValueError('Expected JSON object')
                if path == '/__arena_tools/call':
                    result = self.server.bridge.call(data.get('name'), data.get('arguments', {}), data.get('session'))
                elif path == '/__arena_tools/result':
                    result = self.server.bridge.result(data)
                else:
                    return self.reply(404, {'error': 'Unknown endpoint'})
                self.reply(200, result)
            except (ValueError, UnicodeError, TypeError) as error:
                self.reply(400, {'error': str(error)})
    return Handler


def remote_call(url, token, name, arguments):
    arguments = dict(arguments)
    session = arguments.pop('session', None)
    body = json.dumps(dict(name=name, arguments=arguments, session=session)).encode()
    request = Request(url.rstrip('/') + '/__arena_tools/call', data=body,
                      headers={'Authorization': 'Bearer ' + token, 'Content-Type': 'application/json'})
    try:
        with urlopen(request, timeout=10) as response:
            return json.loads(response.read(MAX_BODY))
    except HTTPError as error:
        with error:
            try:
                return {'error': json.loads(error.read(MAX_BODY)).get('error', str(error))}
            except (ValueError, UnicodeError):
                return {'error': str(error)}
    except (URLError, TimeoutError, ValueError) as error:
        return {'error': str(error)}


def rpc(request, call):
    if not isinstance(request, dict):
        return {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32600, 'message': 'Invalid Request'}}
    if 'id' not in request:
        return None
    response = {'jsonrpc': '2.0', 'id': request['id']}
    method = request.get('method')
    if request.get('jsonrpc') != '2.0' or not isinstance(method, str):
        response['error'] = {'code': -32600, 'message': 'Invalid Request'}
    elif method == 'initialize':
        response['result'] = {'protocolVersion': '2024-11-05', 'capabilities': {'tools': {}},
                              'serverInfo': {'name': 'neon-arena', 'version': '1.0.0'}}
    elif method == 'ping':
        response['result'] = {}
    elif method == 'tools/list':
        response['result'] = {'tools': TOOLS}
    elif method == 'tools/call':
        try:
            params = request.get('params', {})
            if not isinstance(params, dict):
                raise ValueError('Expected tool parameters')
            name, arguments = params.get('name'), params.get('arguments', {})
            validate(name, arguments)
            result = call(name, arguments)
            failed = 'error' in result
            content = result.get('error') if failed else result.get('result')
            response['result'] = {'content': [{'type': 'text', 'text': json.dumps(content, ensure_ascii=False)}], 'isError': failed}
        except (ValueError, TypeError) as error:
            response['result'] = {'content': [{'type': 'text', 'text': str(error)}], 'isError': True}
    else:
        response['error'] = {'code': -32601, 'message': 'Method not found'}
    return response


def mcp_main(url, token):
    while True:
        line = sys.stdin.buffer.readline(MAX_BODY + 1)
        if not line:
            return
        if len(line) > MAX_BODY:
            while line and not line.endswith(b'\n'):
                line = sys.stdin.buffer.readline(MAX_BODY + 1)
            response = {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32700, 'message': 'Message too large'}}
        else:
            try:
                response = rpc(json.loads(line), lambda name, arguments: remote_call(url, token, name, arguments))
            except (ValueError, UnicodeError):
                response = {'jsonrpc': '2.0', 'id': None, 'error': {'code': -32700, 'message': 'Parse error'}}
        if response is not None:
            print(json.dumps(response, ensure_ascii=False), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('--mcp', action='store_true', help='run JSONL MCP stdio client (no HTTP listener)')
    parser.add_argument('--url', default='http://127.0.0.1:8766', help='bridge origin for MCP mode')
    parser.add_argument('--token', required=True, help='shared private bearer token')
    parser.add_argument('--host', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=8766)
    parser.add_argument('--directory', default=str(Path(__file__).resolve().parents[1]))
    parser.add_argument('--production', action='store_true', help='reuse precompressed production static handler')
    parser.add_argument('--cert')
    parser.add_argument('--key')
    args = parser.parse_args()
    if not args.token or '\n' in args.token or '\r' in args.token:
        parser.error('token must be nonempty and single-line')
    if args.mcp:
        parsed = urlsplit(args.url)
        if parsed.scheme not in ('http', 'https') or not parsed.netloc or parsed.username or parsed.password or parsed.query or parsed.fragment or parsed.path not in ('', '/'):
            parser.error('--url must be an HTTP(S) bridge origin')
        return mcp_main(args.url, args.token)
    if bool(args.cert) != bool(args.key) or not 0 <= args.port <= 65535:
        parser.error('provide --cert and --key together, and a port from 0 to 65535')
    with ThreadingHTTPServer((args.host, args.port), partial(handler_class(args.production), directory=args.directory)) as server:
        server.token, server.bridge = args.token, Bridge()
        if args.cert:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(args.cert, args.key)
            server.socket = context.wrap_socket(server.socket, server_side=True)
        print(f"{'https' if args.cert else 'http'}://{args.host}:{server.server_port}", flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == '__main__':
    main()
