"""A small persistent, scores-only HTTP scoreboard for the Neon arena."""
import json
import os
import re
from contextlib import contextmanager
import fcntl
from pathlib import Path
import tempfile
import threading
from urllib.parse import urlsplit


MAX_SCORE = 9007199254740991
MAX_BODY = 1024
SCOREBOARD_HTML = '''<!doctype html>
<html lang="cs"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Nejvyšší skóre</title><style>
body{margin:0;background:#10111d;color:#f0faff;font:24px system-ui,sans-serif}
main{max-width:760px;margin:5vh auto;padding:24px}h1{font-size:clamp(32px,6vw,58px);color:#7afaff}
ol{padding-left:2.8em;font-size:clamp(32px,6vw,58px);font-variant-numeric:tabular-nums}
li{padding:10px 18px;border-bottom:1px solid #344054}li:first-child{color:#e4a7ff}
p{color:#b8c9dc}</style><main><h1>Nejvyšší skóre</h1><p id="status">Načítám skóre…</p><ol id="scores"></ol></main>
<script>
async function refresh(){try{
 const response=await fetch('/__neon_leaderboard_v2',{cache:'no-store'});
 if(!response.ok)throw new Error();
 const scores=await response.json();
 const rows=scores.map(score=>{const row=document.createElement('li');row.textContent=`${score.initials}  ${score.score}`;return row});
 document.getElementById('scores').replaceChildren(...rows);
 document.getElementById('status').textContent=scores.length?'':'Zatím žádné skóre.';
}catch{document.getElementById('status').textContent='Skóre se nepodařilo načíst.'}}
refresh();setInterval(refresh,3000);
</script></html>'''.encode('utf-8')


def valid_score(value):
    return type(value) is int and 0 <= value <= MAX_SCORE


def valid_entry(value):
    return (type(value) is dict and valid_score(value.get('score')) and value['score'] > 0
            and type(value.get('initials')) is str and re.fullmatch(r'[A-Z]{1,3}', value['initials']) is not None)


def make_scoreboard_handler(base_handler, score_path):
    """Add scoreboard routes without changing the supplied handler's other paths."""
    score_path = Path(score_path)
    named_path = score_path.with_name(score_path.stem + '.v2' + score_path.suffix)
    lock = threading.Lock()

    @contextmanager
    def locked_store():
        # A stable sidecar inode keeps the lock valid when atomic replacement
        # changes the scoreboard inode, across factories and server processes.
        with lock:
            score_path.parent.mkdir(parents=True, exist_ok=True)
            with (score_path.parent / (score_path.name + '.lock')).open('a') as stream:
                fcntl.flock(stream.fileno(), fcntl.LOCK_EX)
                try:
                    yield
                finally:
                    fcntl.flock(stream.fileno(), fcntl.LOCK_UN)

    def load_scores():
        try:
            data = json.loads(score_path.read_text(encoding='utf-8'))
        except FileNotFoundError:
            return []
        if type(data) is not list or not all(valid_score(score) for score in data):
            raise ValueError('Existing scoreboard is not a numeric score array')
        return sorted(data, reverse=True)

    def load_named():
        try:
            data = json.loads(named_path.read_text(encoding='utf-8'))
        except FileNotFoundError:
            return {'version': 2, 'entries': [], 'requests': []}
        if (type(data) is not dict or data.get('version') != 2
                or type(data.get('entries')) is not list or len(data['entries']) > 10
                or not all(valid_entry(entry) for entry in data['entries'])
                or type(data.get('requests')) is not list
                or not all(type(key) is str for key in data['requests'])):
            raise ValueError('Invalid named leaderboard')
        return data

    def save_scores(scores, destination=score_path):
        score_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = None
        try:
            with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8',
                    dir=score_path.parent, prefix='.' + score_path.name + '-', suffix='.tmp', delete=False) as stream:
                temporary = stream.name
                json.dump(scores, stream, separators=(',', ':'))
                stream.write('\n')
                stream.flush()
                os.fsync(stream.fileno())
            os.replace(temporary, destination)
        finally:
            if temporary is not None:
                try:
                    os.unlink(temporary)
                except FileNotFoundError:
                    pass

    class ScoreboardHandler(base_handler):
        def respond(self, status, payload, content_type='application/json; charset=utf-8', head=False):
            self.send_response(status)
            self.send_header('Content-Type', content_type)
            self.send_header('Content-Length', str(len(payload)))
            self.send_header('Cache-Control', 'no-store')
            self.end_headers()
            if not head:
                self.wfile.write(payload)

        def scoreboard_get(self, head=False):
            route = urlsplit(self.path).path
            if route == '/scoreboard':
                self.respond(200, SCOREBOARD_HTML, 'text/html; charset=utf-8', head)
                return True
            if route not in ('/__neon_scores', '/__neon_leaderboard_v2'):
                return False
            try:
                with locked_store():
                    scores = load_named()['entries'] if route == '/__neon_leaderboard_v2' else load_scores()
                self.respond(200, json.dumps(scores).encode('utf-8'), head=head)
            except (OSError, ValueError, UnicodeError):
                self.respond(500, b'[]', head=head)
            return True

        def do_GET(self):
            if not self.scoreboard_get():
                super().do_GET()

        def do_HEAD(self):
            if not self.scoreboard_get(head=True):
                super().do_HEAD()

        def do_POST(self):
            route = urlsplit(self.path).path
            named = route == '/__neon_leaderboard_v2'
            if route not in ('/__neon_scores', '/__neon_leaderboard_v2'):
                super().do_POST()
                return
            try:
                length = int(self.headers.get('Content-Length', '0'))
                if not 0 < length <= MAX_BODY or self.headers.get_content_type() != 'application/json':
                    raise ValueError('Invalid request size or content type')
                payload = json.loads(self.rfile.read(length))
                if named:
                    if (type(payload) is not dict or set(payload) != {'score', 'initials', 'requestId'}
                            or not valid_entry(payload) or type(payload['requestId']) is not str
                            or not re.fullmatch(r'[A-Za-z0-9:._-]{1,128}', payload['requestId'])):
                        raise ValueError('Invalid named score')
                elif type(payload) is not dict or set(payload) != {'score'} or not valid_score(payload['score']):
                    raise ValueError('Invalid score')
            except (ValueError, UnicodeError):
                self.respond(400, b'[]')
                return
            try:
                with locked_store():
                    if named:
                        data = load_named()
                        if payload['requestId'] not in data['requests']:
                            data['entries'].append({'score': payload['score'], 'initials': payload['initials']})
                            data['entries'].sort(key=lambda entry: entry['score'], reverse=True)
                            data['entries'] = data['entries'][:10]
                            data['requests'] = (data['requests'] + [payload['requestId']])[-1000:]
                            save_scores(data, named_path)
                        scores = data['entries']
                    else:
                        scores = load_scores()
                        scores.append(payload['score'])
                        scores.sort(reverse=True)
                        save_scores(scores)
                self.respond(201, json.dumps(scores).encode('utf-8'))
            except (OSError, ValueError, UnicodeError):
                self.respond(500, b'[]')

    return ScoreboardHandler
