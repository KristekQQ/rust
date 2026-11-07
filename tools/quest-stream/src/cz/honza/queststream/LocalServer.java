package cz.honza.queststream;

import java.io.*;
import java.net.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;

/** Small bounded HTTP/1.1 server. A stream occupies one worker, never the accept loop. */
public final class LocalServer implements AutoCloseable {
    public interface Handler { void handle(String path, Socket socket) throws Exception; }
    private final ServerSocket server;
    private final ThreadPoolExecutor workers = new ThreadPoolExecutor(3, 3, 0, TimeUnit.SECONDS,
        new ArrayBlockingQueue<Runnable>(4), new ThreadPoolExecutor.AbortPolicy());
    private volatile boolean closed;
    private final java.util.Set<Socket> peers = java.util.concurrent.ConcurrentHashMap.newKeySet();
    public LocalServer(int port, Handler handler) throws IOException {
        server = new ServerSocket(); server.setReuseAddress(true); server.bind(new InetSocketAddress(port));
        Thread accept = new Thread(() -> {
            while (!closed) {
                try {
                    Socket socket = server.accept();
                    peers.add(socket);
                    if (closed) { socket.close(); peers.remove(socket); break; }
                    socket.setSoTimeout(5000); socket.setTcpNoDelay(true); socket.setSendBufferSize(65536);
                    try { workers.execute(() -> {
                        try (Socket peer = socket) {
                            InputStream input = peer.getInputStream();
                            ByteArrayOutputStream header = new ByteArrayOutputStream();
                            int tail = 0;
                            while (header.size() < 8192) {
                                int b = input.read(); if (b < 0) return;
                                header.write(b); tail = (tail << 8) | b;
                                if (tail == 0x0d0a0d0a) break;
                            }
                            if (tail != 0x0d0a0d0a) { reply(peer, 431, "text/plain", "Header too large".getBytes(StandardCharsets.UTF_8)); return; }
                            String first = header.toString("US-ASCII").split("\r\n", 2)[0];
                            String[] parts = first.split(" ");
                            if (parts.length != 3 || !parts[0].equals("GET")) { reply(peer, 405, "text/plain", new byte[0]); return; }
                            handler.handle(parts[1], peer);
                        } catch (Exception ignored) { /* Disconnects are normal; capture handler owns its status. */ }
                        finally { peers.remove(socket); }
                    }); } catch (RejectedExecutionException busy) { socket.close(); peers.remove(socket); }
                } catch (IOException e) { if (!closed) close(); }
            }
        }, "quest-http"); accept.setDaemon(true); accept.start();
    }
    public int port() { return server.getLocalPort(); }
    public static void reply(Socket socket, int code, String type, byte[] data) throws IOException {
        OutputStream out = socket.getOutputStream();
        String headers = "HTTP/1.1 " + code + " Response\r\nContent-Type: " + type + "\r\nContent-Length: " + data.length
            + "\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nX-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n";
        out.write(headers.getBytes(StandardCharsets.US_ASCII)); out.write(data); out.flush();
    }
    public void close() {
        closed = true;
        try { server.close(); } catch (IOException ignored) {}
        workers.shutdownNow();
        for (Socket peer : peers) try { peer.close(); } catch (IOException ignored) {}
        peers.clear();
    }
}
