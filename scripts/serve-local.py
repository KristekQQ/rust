#!/usr/bin/env python3
"""Static development server with revalidated module responses."""
import argparse
import ssl
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer


class DevelopmentHandler(SimpleHTTPRequestHandler):
    def do_GET(self):
        if self.path.split('?', 1)[0] == '/favicon.ico':
            self.send_response(204)
            self.end_headers()
            return
        super().do_GET()

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
    parser.add_argument('--host', default='127.0.0.1', help='bind address (default: loopback only)')
    parser.add_argument('--cert', help='TLS certificate PEM; requires --key')
    parser.add_argument('--key', help='TLS private key PEM; requires --cert')
    args = parser.parse_args()
    if not 0 <= args.port <= 65535:
        parser.error('port must be between 0 and 65535 (0 selects a free port)')
    if bool(args.cert) != bool(args.key):
        parser.error('--cert and --key must be supplied together')
    with ThreadingHTTPServer((args.host, args.port), DevelopmentHandler) as server:
        scheme = 'https' if args.cert else 'http'
        if args.cert:
            context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
            context.load_cert_chain(args.cert, args.key)
            server.socket = context.wrap_socket(server.socket, server_side=True)
        print(f'{scheme}://{args.host}:{server.server_port}', flush=True)
        try:
            server.serve_forever()
        except KeyboardInterrupt:
            pass
