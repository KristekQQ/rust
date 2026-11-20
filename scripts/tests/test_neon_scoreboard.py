"""HTTP integration checks with isolated stores; never touch live scores."""
import concurrent.futures
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from neon_scoreboard import make_scoreboard_handler


class QuietHandler(SimpleHTTPRequestHandler):
    def log_message(self, *_args):
        pass


class NamedLeaderboardTest(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.path = Path(self.temp.name) / 'neon-arena.json'
        self.path.write_text('[99999]')
        self.server = ThreadingHTTPServer(('127.0.0.1', 0), make_scoreboard_handler(QuietHandler, self.path))
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join()
        self.temp.cleanup()

    def request(self, payload=None, route='/__neon_leaderboard_v2'):
        data = None if payload is None else json.dumps(payload).encode()
        request = Request(f'http://127.0.0.1:{self.server.server_port}{route}', data=data,
                          headers={'Content-Type': 'application/json'})
        try:
            with urlopen(request) as response:
                return response.status, json.loads(response.read())
        except HTTPError as response:
            with response:
                return response.code, json.loads(response.read())

    def test_fresh_version_preserves_old_anonymous_scores(self):
        self.assertEqual(self.request(), (200, []))
        self.assertEqual(self.request({'score': 5, 'initials': 'HZ', 'requestId': 'first'}),
                         (201, [{'score': 5, 'initials': 'HZ'}]))
        self.assertEqual(self.path.read_text(), '[99999]')
        self.assertEqual(self.request(route='/__neon_scores'), (200, [99999]))
        named = self.path.with_name('neon-arena.v2.json')
        self.assertEqual(json.loads(named.read_text())['version'], 2)
        # A new server factory reads the same versioned file, not a process cache.
        other = ThreadingHTTPServer(('127.0.0.1', 0), make_scoreboard_handler(QuietHandler, self.path))
        thread = threading.Thread(target=other.handle_request)
        thread.start()
        try:
            with urlopen(f'http://127.0.0.1:{other.server_port}/__neon_leaderboard_v2') as response:
                self.assertEqual(json.load(response), [{'score': 5, 'initials': 'HZ'}])
        finally:
            thread.join()
            other.server_close()

    def test_top10_and_concurrent_duplicate_confirmation(self):
        payloads = [{'score': score, 'initials': 'A', 'requestId': str(score)} for score in range(1, 14)]
        payloads += [payloads[-1]] * 4
        with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:
            responses = list(pool.map(self.request, payloads))
        self.assertTrue(all(status == 201 for status, _ in responses))
        self.assertEqual([entry['score'] for entry in self.request()[1]], list(range(13, 3, -1)))

    def test_invalid_entries_do_not_mutate_store(self):
        for payload in [{'score': 0, 'initials': 'A', 'requestId': 'x'},
                        {'score': True, 'initials': 'A', 'requestId': 'x'},
                        {'score': 2**53, 'initials': 'A', 'requestId': 'x'},
                        {'score': 1, 'initials': 'ABCD', 'requestId': 'x'},
                        {'score': 1, 'initials': 'a', 'requestId': 'x'},
                        {'score': 1, 'initials': 'A'},
                        {'score': 1, 'initials': 'A', 'requestId': '<script>'}]:
            self.assertEqual(self.request(payload)[0], 400)
        self.assertEqual(self.request(), (200, []))


if __name__ == '__main__':
    unittest.main()
