#!/usr/bin/env python3
"""Loopback-only static development server with revalidated module responses."""
import argparse
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class DevelopmentHandler(SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cache-Control', 'no-store, no-cache, must-revalidate')
        self.send_header('Pragma', 'no-cache')
        self.send_header('Expires', '0')
        super().end_headers()

    def send_head(self):
        # A development rebuild must never return an old module via a 304 response.
        for name in ('If-Modified-Since', 'If-None-Match'):
            if name in self.headers:
                del self.headers[name]
        return super().send_head()


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('port', nargs='?', type=int, default=8000)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535:
        parser.error('port must be between 1 and 65535')
    with ThreadingHTTPServer(('127.0.0.1', args.port), DevelopmentHandler) as server:
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
