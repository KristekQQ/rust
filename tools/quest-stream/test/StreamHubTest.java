import cz.honza.queststream.StreamHub;
import java.net.Socket;
import java.util.Arrays;

public class StreamHubTest {
    private static void check(boolean condition, String message) {
        if (!condition) throw new AssertionError(message);
    }
    public static void main(String[] args) throws Exception {
        StreamHub hub = new StreamHub(2);
        byte[] init = {1}, key = {2}, delta = {3};
        hub.initialize(init);
        StreamHub.Subscription first = hub.subscribe(new Socket());
        hub.publish(key, true);
        check(Arrays.equals(first.poll(), init), "first starts with init");
        check(Arrays.equals(first.poll(), key), "first starts at key frame");
        StreamHub.Subscription second = hub.subscribe(new Socket());
        check(hub.subscribe(new Socket()) == null, "third reader rejected");
        hub.publish(delta, false);
        check(Arrays.equals(first.poll(), delta), "existing viewer continues");
        hub.publish(key, true);
        check(Arrays.equals(second.poll(), init), "late reader gets init before data");
        check(Arrays.equals(second.poll(), key), "late reader skips pre-key deltas");
        first.close();
        StreamHub.Subscription reconnected = hub.subscribe(new Socket());
        check(reconnected != null, "reconnect uses existing encoder session");
        hub.publish(key, true);
        check(Arrays.equals(reconnected.poll(), init), "reconnect gets init");
        check(Arrays.equals(reconnected.poll(), key), "reconnect gets key");
        second.poll();
        for (int i=0; i<20; i++) {
            hub.publish(delta, false);
            check(Arrays.equals(reconnected.poll(), delta), "fast viewer survives slow viewer");
        }
        check(second.closed(), "slow viewer evicted");
        check(!reconnected.closed(), "healthy viewer stays open");
        hub.close();
        check(reconnected.closed(), "shutdown closes peer");
        check(hub.subscribe(new Socket()) == null, "shutdown rejects new peers");
        System.out.println("PASS: late join, reconnect, independent viewers, bounded backpressure, shutdown");
    }
}
