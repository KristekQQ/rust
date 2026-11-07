import cz.honza.queststream.LocalServer;
import java.net.*;
import java.io.*;
import java.util.*;
public class LocalServerTest {
    public static void main(String[] args) throws Exception {
        LocalServer server = new LocalServer(0,(path,socket) -> LocalServer.reply(socket,200,"text/plain",path.getBytes("UTF-8")));
        try (Socket peer = new Socket("127.0.0.1",server.port())) {
            peer.getOutputStream().write("GET /hello HTTP/1.1\r\nHost: localhost\r\n\r\n".getBytes("US-ASCII"));
            String response = new String(peer.getInputStream().readAllBytes(),"UTF-8");
            if (!response.endsWith("/hello") || !response.startsWith("HTTP/1.1 200")) throw new AssertionError(response);
        }
        List<Socket> peers = new ArrayList<>();
        for (int i=0;i<7;i++) peers.add(new Socket("127.0.0.1",server.port()));
        Thread.sleep(100);
        server.close();
        for(Socket peer:peers) {
            peer.setSoTimeout(1000);
            if(peer.getInputStream().read()!=-1) throw new AssertionError("Peer left open");
            peer.close();
        }
        System.out.println("PASS HTTP response and shutdown closes active/queued peers");
    }
}
