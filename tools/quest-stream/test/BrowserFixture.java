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
                long started = System.nanoTime(); long lastJitterSecond = 0;
                for (int p = 0; p < mp4.length;) {
                    int size = ByteBuffer.wrap(mp4,p,4).getInt();
                    String type = new String(mp4,p+4,4,"US-ASCII");
                    if (type.equals("moof")) {
                        int track=1; long timestamp=0;
                        for(int i=p+4;i<p+size-12;i++) {
                            String tag=new String(mp4,i,4,"US-ASCII");
                            if(tag.equals("tfhd")) track=ByteBuffer.wrap(mp4,i+8,4).getInt();
                            if(tag.equals("tfdt")) timestamp=ByteBuffer.wrap(mp4,i+8,8).getLong();
                        }
                        long nanos=timestamp*1000000000L/(track==2?48000:1000000);
                        long waitMs=(started+nanos-System.nanoTime())/1000000;
                        if(waitMs>0) Thread.sleep(waitMs);
                        long second=nanos/1000000000;
                        if(second>lastJitterSecond && jitterMs>0) Thread.sleep(jitterMs);
                        lastJitterSecond=second;
                    }
                    out.write(mp4,p,size); out.flush();
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
