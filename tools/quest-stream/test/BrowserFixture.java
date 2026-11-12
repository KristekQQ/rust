import cz.honza.queststream.LocalServer;
import java.nio.file.*;
import java.io.*;
import java.nio.*;
/** Desktop fixture for the actual HTTP handler + shipped viewer (no Android capture). */
public class BrowserFixture {
    public static void main(String[] args) throws Exception {
        Path root = Paths.get(args[0]); byte[] mp4 = Files.readAllBytes(root.resolve("build/fixture.mp4"));
        int jitterMs = Integer.parseInt(System.getenv().getOrDefault("QUEST_STREAM_FIXTURE_JITTER_MS", "0"));
        if (jitterMs < 0 || jitterMs > 400) throw new IllegalArgumentException("Fixture jitter must be 0..400 ms");
        LocalServer server = new LocalServer(0, (path, socket) -> {
            String uri = path.split("\\?",2)[0];
            if (uri.equals("/stream")) {
                OutputStream out = socket.getOutputStream();
                out.write("HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nConnection: close\r\n\r\n".getBytes("US-ASCII"));
                long started = System.nanoTime(); int frames = 0;
                for (int p = 0; p < mp4.length;) {
                    int size = ByteBuffer.wrap(mp4,p,4).getInt();
                    out.write(mp4,p,size); out.flush();
                    String type = new String(mp4,p+4,4,"US-ASCII");
                    if (type.equals("mdat")) {
                        frames++;
                        long waitMs = (started + frames * 1_000_000_000L / 30 - System.nanoTime()) / 1_000_000;
                        if (waitMs > 0) Thread.sleep(waitMs);
                        if (frames % 30 == 0 && jitterMs > 0) Thread.sleep(jitterMs);
                    }
                    p += size;
                }
                Thread.sleep(15000);
            } else if (uri.equals("/") || uri.equals("/viewer.js") || uri.equals("/playback-policy.js")) {
                LocalServer.reply(socket,200,uri.equals("/")?"text/html; charset=utf-8":"text/javascript", Files.readAllBytes(root.resolve("assets/"+(uri.equals("/")?"index.html":uri.substring(1)))));
            } else LocalServer.reply(socket,404,"text/plain",new byte[0]);
        });
        System.out.println("FIXTURE http://127.0.0.1:" + server.port() + "/?token=fixture");
        Runtime.getRuntime().addShutdownHook(new Thread(server::close));
        Thread.sleep(600000);
        server.close();
    }
}
