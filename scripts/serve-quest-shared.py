#!/usr/bin/env python3
"""Keep an existing Quest release available while mounting a separate arena build."""
import argparse
from functools import partial
from http.server import ThreadingHTTPServer
import importlib.util
import os
from pathlib import Path
import ssl
from urllib.parse import urlsplit, urlunsplit


def make_handler(base_handler, arena_directory):
    arena_directory = str(Path(arena_directory).resolve())

    class SharedQuestHandler(base_handler):
        def redirect_arena(self):
            parts = urlsplit(self.path)
            if parts.path == '/examples/neon-arena' or parts.path.startswith('/examples/neon-arena/'):
                self.send_response(307)
                self.send_header('Location', urlunsplit(('', '', '/_neon' + parts.path, parts.query, '')))
                self.send_header('Content-Length', '0')
                self.end_headers()
                return True
            return False

        def do_GET(self):
            if not self.redirect_arena():
                super().do_GET()

        def do_HEAD(self):
            if not self.redirect_arena():
                super().do_HEAD()

        def send_head(self):
            if urlsplit(self.path).path.startswith('/_neon/'):
                target = Path(self.translate_path(self.path)).resolve()
                payload = target / 'index.html' if target.is_dir() else target
                candidates = [target, payload, Path(str(payload) + '.gz'), Path(str(payload) + '.br')]
                if any(not candidate.resolve().is_relative_to(Path(arena_directory)) for candidate in candidates):
                    self.send_error(403)
                    return None
            return super().send_head()

        def translate_path(self, request_path):
            if urlsplit(request_path).path.startswith('/_neon/'):
                original_directory = self.directory
                self.directory = arena_directory
                try:
                    return super().translate_path(request_path[len('/_neon'):])
                finally:
                    self.directory = original_directory
            return super().translate_path(request_path)

    return SharedQuestHandler


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('port', type=int)
    parser.add_argument('--host', required=True)
    parser.add_argument('--cert', required=True)
    parser.add_argument('--key', required=True)
    parser.add_argument('--fallback-root', type=Path, required=True)
    parser.add_argument('--arena-root', type=Path, required=True)
    parser.add_argument('--score-file', type=Path)
    args = parser.parse_args()
    fallback_root = args.fallback_root.resolve()
    arena_root = args.arena_root.resolve()
    if not (fallback_root / 'output/production/production-manifest.json').is_file():
        parser.error('Fallback root must contain an existing production release.')
    if not (arena_root / 'production-manifest.json').is_file():
        parser.error('Arena root must contain a production release.')
    cert, key = str(Path(args.cert).resolve()), str(Path(args.key).resolve())
    spec = importlib.util.spec_from_file_location('quest_fallback_server', fallback_root / 'scripts/serve-local.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    score_spec = importlib.util.spec_from_file_location('neon_scoreboard', Path(__file__).with_name('neon_scoreboard.py').resolve())
    score_module = importlib.util.module_from_spec(score_spec)
    score_spec.loader.exec_module(score_module)
    score_file = args.score_file.resolve() if args.score_file else arena_root.parent / 'scoreboard/neon-arena.json'
    shared_handler = make_handler(module.ProductionHandler, arena_root)
    shared_handler = score_module.make_scoreboard_handler(shared_handler, score_file)
    # Preserve the existing server's diagnostics destination as well as its files.
    os.chdir(fallback_root)
    handler = partial(shared_handler, directory=str(fallback_root / 'output/production'))
    with ThreadingHTTPServer((args.host, args.port), handler) as server:
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        print(f'https://{args.host}:{args.port} (existing release + current neon arena)', flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass


if __name__ == '__main__':
    main()
