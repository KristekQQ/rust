"""Exercise actual HTTP delivery of precompressed production bytes."""
import gzip
import http.client
import importlib.util
from functools import partial
from http.server import ThreadingHTTPServer
from pathlib import Path
import tempfile
import threading
import unittest

spec = importlib.util.spec_from_file_location('server', Path(__file__).parents[1] / 'scripts/serve-local.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class ProductionDelivery(unittest.TestCase):
    def test_compression_negotiation_and_identity(self):
        with tempfile.TemporaryDirectory() as directory:
            source = b'export const value = 123;\n'
            target = Path(directory) / 'main.js'
            target.write_bytes(source)
            target.with_suffix('.js.gz').write_bytes(gzip.compress(source))
            # The server forwards br bytes; Node packaging tests validate Brotli decoding.
            target.with_suffix('.js.br').write_bytes(b'precompressed-br')
            page = Path(directory) / 'examples/demo'
            page.mkdir(parents=True)
            (page / 'index.html').write_text('<script src="./main.js"></script>')
            (page / 'index.html.gz').write_bytes(gzip.compress(b'<html>demo</html>'))
            handler = partial(module.ProductionHandler, directory=directory)
            with ThreadingHTTPServer(('127.0.0.1', 0), handler) as server:
                worker = threading.Thread(target=server.serve_forever, daemon=True)
                worker.start()
                try:
                    connection = http.client.HTTPConnection(*server.server_address)
                    connection.request('GET', '/examples/demo', headers={'Accept-Encoding': 'gzip'})
                    response = connection.getresponse()
                    self.assertEqual(response.status, 301)
                    self.assertEqual(response.getheader('Location'), '/examples/demo/')
                    response.read()
                    connection.close()
                    for accepted, encoding in [('br, gzip', 'br'), ('br;q=0,gzip;q=1', 'gzip'), ('identity', None)]:
                        connection = http.client.HTTPConnection(*server.server_address)
                        connection.request('GET', '/main.js?v=1', headers={'Accept-Encoding': accepted})
                        response = connection.getresponse()
                        body = response.read()
                        self.assertEqual(response.status, 200)
                        self.assertIn('javascript', response.getheader('Content-Type'))
                        self.assertEqual(response.getheader('Content-Encoding'), encoding)
                        if encoding == 'gzip':
                            self.assertEqual(gzip.decompress(body), source)
                        elif encoding is None:
                            self.assertEqual(body, source)
                        self.assertIn('no-store', response.getheader('Cache-Control'))
                        connection.close()
                finally:
                    server.shutdown()
                    worker.join()

    def test_plain_refresh_canonical_url_and_conditional_requests(self):
        for handler_type in [module.DevelopmentHandler, module.ProductionHandler]:
            with self.subTest(handler=handler_type.__name__), tempfile.TemporaryDirectory() as directory:
                page = Path(directory) / 'examples/rim'
                page.mkdir(parents=True)
                target = page / 'index.html'
                target.write_bytes(b'first application')
                with ThreadingHTTPServer(('127.0.0.1', 0), partial(handler_type, directory=directory)) as server:
                    worker = threading.Thread(target=server.serve_forever, daemon=True)
                    worker.start()
                    try:
                        def request(path, headers=None):
                            connection = http.client.HTTPConnection(*server.server_address)
                            connection.request('GET', path, headers=headers or {})
                            response = connection.getresponse()
                            result = response.status, dict(response.getheaders()), response.read()
                            connection.close()
                            return result
                        status, headers, _ = request('/examples/rim/?backend=webgpu&v=7c37709d6c&profile=0&view=gate')
                        self.assertEqual(status, 302)
                        self.assertEqual(headers['Location'], '/examples/rim/?backend=webgpu&profile=0&view=gate')
                        self.assertIn('no-store', headers['Cache-Control'])
                        path = '/examples/rim/?backend=webgpu'
                        status, headers, body = request(path)
                        self.assertEqual((status, body), (200, b'first application'))
                        target.write_bytes(b'current application')
                        status, headers, body = request(path, {
                            'If-Modified-Since': 'Thu, 01 Jan 2099 00:00:00 GMT',
                            'If-None-Match': 'old-version', 'Cache-Control': 'max-age=0',
                        })
                        self.assertEqual((status, body), (200, b'current application'))
                        self.assertIn('no-store', headers['Cache-Control'])
                        self.assertEqual(headers['Pragma'], 'no-cache')
                        self.assertEqual(headers['Expires'], '0')
                        self.assertNotIn('Clear-Site-Data', headers)
                    finally:
                        server.shutdown()
                        worker.join()


if __name__ == '__main__':
    unittest.main()
