// Local companion for the Vesper dev server at 127.0.0.1:8742.
const { WebSocketServer, WebSocket } = require('ws');
const { randomUUID } = require('node:crypto');
const { execFile } = require('node:child_process');

const origin = 'http://127.0.0.1:8742';
const sessions = new Map();
const server = new WebSocketServer({
  host: '127.0.0.1', port: 8743, maxPayload: 4096,
  verifyClient: ({ origin: requestOrigin }) => requestOrigin === origin,
});

server.on('connection', (socket, request) => {
  const route = new URL(request.url, origin);
  if (route.pathname === '/open') {
    const key = randomUUID();
    const session = { source: socket, viewer: null };
    sessions.set(key, session);
    const url = new URL('/vesper/', origin);
    url.searchParams.set('scene', route.searchParams.get('scene') === 'kral' ? 'kral' : 'vir');
    if (route.searchParams.get('kamera') === 'fraze') url.searchParams.set('kamera', 'fraze');
    url.hash = 'projekce=' + key;
    execFile('/usr/bin/open', ['-a', 'Safari', url.href], error => {
      if (error) socket.close(1011, 'open');
    });
    socket.on('message', (data, binary) => {
      if (!binary && session.viewer?.readyState === WebSocket.OPEN) session.viewer.send(data.toString());
    });
    socket.on('close', () => {
      sessions.delete(key);
      session.viewer?.close();
    });
    return;
  }

  const session = sessions.get(route.pathname.slice(1));
  if (!session || session.viewer) return socket.close(1008);
  session.viewer = socket;
  socket.on('message', data => {
    if (data.toString() === 'frame' && session.source.readyState === WebSocket.OPEN) session.source.send('frame');
  });
  socket.on('close', () => session.source.close());
});
