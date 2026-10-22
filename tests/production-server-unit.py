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


if __name__ == '__main__':
    unittest.main()
