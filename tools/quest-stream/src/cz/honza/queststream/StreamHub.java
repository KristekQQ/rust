package cz.honza.queststream;

import java.io.IOException;
import java.net.Socket;
import java.util.HashSet;
import java.util.Set;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.TimeUnit;

/** One encoder session, bounded independent readers. A new reader starts at a key frame. */
public final class StreamHub implements AutoCloseable {
    private final int limit;
    private final Set<Subscription> readers = new HashSet<>();
    private byte[] init;
    private boolean closed;
    private int packetLimit = 16;
    public StreamHub(int limit) { this.limit = limit; }
    public synchronized Subscription subscribe(Socket socket) {
        if (closed || readers.size() >= limit) return null;
        Subscription reader = new Subscription(socket);
        readers.add(reader);
        return reader;
    }
    public synchronized void initialize(byte[] data) { initialize(data, false); }
    public synchronized void initialize(byte[] data, boolean audio) { init = data; packetLimit = audio ? 96 : 16; }
    public synchronized void publish(byte[] data, boolean keyFrame) { publish(data, keyFrame, -1, false); }
    public synchronized void publish(byte[] data, boolean keyFrame, long ptsUs, boolean audio) {
        for (Subscription reader : new HashSet<>(readers)) {
            if (reader.waitingForKey) {
                if (!keyFrame || init == null) continue;
                reader.queue.offer(init);
                reader.waitingForKey = false;
                reader.keyPtsUs = ptsUs;
            }
            if (audio && ptsUs < reader.keyPtsUs) continue;
            // A slow/disconnected browser must never stop other browsers or the encoder.
            if (reader.queue.size() >= packetLimit || !reader.queue.offer(data)) reader.close();
        }
    }
    /** Change optional tracks after an audio error. Readers reconnect to a fresh init. */
    public synchronized void reinitialize(byte[] data) {
        for (Subscription reader : new HashSet<>(readers)) reader.close();
        init = data;
        packetLimit = 16;
    }
    public synchronized int viewers() { return readers.size(); }
    public synchronized void close() {
        closed = true;
        for (Subscription reader : new HashSet<>(readers)) reader.close();
        init = null;
    }
    public final class Subscription implements AutoCloseable {
        private final Socket socket;
        private final ArrayBlockingQueue<byte[]> queue = new ArrayBlockingQueue<>(96);
        private boolean waitingForKey = true;
        private long keyPtsUs = -1;
        private volatile boolean closed;
        private Subscription(Socket socket) { this.socket = socket; }
        public byte[] poll() throws InterruptedException { return queue.poll(1, TimeUnit.SECONDS); }
        public boolean closed() { return closed; }
        public void close() {
            synchronized (StreamHub.this) {
                if (closed) return;
                closed = true;
                readers.remove(this);
                queue.clear();
            }
            try { socket.close(); } catch (IOException ignored) {}
        }
    }
}
