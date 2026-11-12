package cz.honza.queststream;

import android.app.*;
import android.content.*;
import android.content.pm.ServiceInfo;
import android.hardware.display.*;
import android.media.*;
import android.media.projection.*;
import android.os.*;
import android.view.Surface;
import java.io.*;
import java.net.*;
import java.nio.*;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.*;


public final class StreamService extends Service {
    public static volatile String status = "Přenos je vypnutý.";
    public static volatile String token;
    private static volatile boolean capturing;
    public static boolean isCapturing() { return capturing; }
    private LocalServer server;
    private MediaProjection projection;
    private volatile boolean destroyed;
    private final StreamHub hub = new StreamHub(2);
    private volatile boolean captureDone;
    private int width = 1280, height = 720, fps = 30, bitrate = 3000000;
    public IBinder onBind(Intent i) { return null; }
    public int onStartCommand(Intent intent, int flags, int id) {
        if (intent != null && "stop".equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        if (projection != null) return START_NOT_STICKY;
        try {
            NotificationManager nm = getSystemService(NotificationManager.class);
            nm.createNotificationChannel(new NotificationChannel("stream", "Quest Stream", NotificationManager.IMPORTANCE_LOW));
            PendingIntent stop = PendingIntent.getService(this, 1, new Intent(this, StreamService.class).setAction("stop"), PendingIntent.FLAG_IMMUTABLE);
            PendingIntent open = PendingIntent.getActivity(this, 2, new Intent(this, MainActivity.class), PendingIntent.FLAG_IMMUTABLE);
            Notification n = new Notification.Builder(this, "stream").setContentTitle("Quest Stream")
                .setContentText("Sdílení obrazovky přes místní síť").setSmallIcon(android.R.drawable.presence_video_online)
                .setContentIntent(open).setOngoing(true).addAction(new Notification.Action.Builder(null, "Ukončit", stop).build()).build();
            startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROJECTION);
            if (intent == null) throw new IllegalStateException("Chybí souhlas se sdílením.");
            Intent consent = intent.getParcelableExtra("data");
            if (consent == null) throw new IllegalStateException("Chybí souhlas se sdílením.");
            token = intent.getStringExtra("token");
            if (token == null || !token.matches("[a-f0-9]{32}")) throw new IllegalStateException("Neplatný přístupový klíč.");
            if (intent.getIntExtra("profile", 0) == 1) { width = 960; height = 540; fps = 24; bitrate = 2000000; }
            projection = getSystemService(MediaProjectionManager.class).getMediaProjection(intent.getIntExtra("result", -1), consent);
            projection.registerCallback(new MediaProjection.Callback() {
                public void onStop() { if (!destroyed) { status = "Systém ukončil sdílení. Povol nový přenos."; stopSelf(); } }
            }, new Handler(getMainLooper()));
            server = new LocalServer(8088, this::request);
            status = "Spouštím sdílení…";
            new Thread(this::capture, "quest-encoder").start();
        } catch (Exception e) { status = "Přenos nelze spustit: " + e.getMessage(); stopSelf(); }
        return START_NOT_STICKY;
    }
    private void request(String path, Socket socket) throws Exception {
        URI uri;
        try { uri = new URI(path); } catch (Exception e) { LocalServer.reply(socket, 400, "text/plain", new byte[0]); return; }
        String key = null;
        if (uri.getRawQuery() != null) for (String item : uri.getRawQuery().split("&"))
            if (item.startsWith("token=")) key = item.substring(6);
        String expected = token;
        if (expected == null || !expected.equals(key)) { LocalServer.reply(socket, 403, "text/plain", "Neplatný odkaz.".getBytes(StandardCharsets.UTF_8)); return; }
        if ("/".equals(uri.getPath()) || "/viewer.js".equals(uri.getPath()) || "/playback-policy.js".equals(uri.getPath())) {
            String name = "/".equals(uri.getPath()) ? "index.html" : uri.getPath().substring(1);
            try (InputStream in = getAssets().open(name); ByteArrayOutputStream b = new ByteArrayOutputStream()) {
                byte[] chunk = new byte[8192]; int count;
                while ((count = in.read(chunk)) != -1) b.write(chunk, 0, count);
                LocalServer.reply(socket, 200, name.endsWith("js") ? "text/javascript; charset=utf-8" : "text/html; charset=utf-8", b.toByteArray());
            }
        } else if ("/stream".equals(uri.getPath())) {
            if (captureDone || destroyed) { LocalServer.reply(socket, 410, "text/plain", status.getBytes(StandardCharsets.UTF_8)); return; }
            StreamHub.Subscription subscription = hub.subscribe(socket);
            if (subscription == null) { LocalServer.reply(socket, 409, "text/plain", "Přenos už sledují dvě okna. Zavři jedno z nich a zkus to znovu.".getBytes(StandardCharsets.UTF_8)); return; }
            try (StreamHub.Subscription peer = subscription) {
                OutputStream out = socket.getOutputStream();
                out.write(("HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n").getBytes(StandardCharsets.US_ASCII));
                out.flush();
                while (!destroyed && !peer.closed()) {
                    byte[] packet = peer.poll();
                    if (packet != null) { out.write(packet); out.flush(); }
                    else if (captureDone) break;
                }
            }
        } else if ("/status".equals(uri.getPath())) {
            LocalServer.reply(socket, 200, "text/plain; charset=utf-8", status.getBytes(StandardCharsets.UTF_8));
        } else { LocalServer.reply(socket, 404, "text/plain", new byte[0]); }
    }
    private static byte[] bytes(ByteBuffer b) {
        if (b == null) throw new IllegalStateException("Enkodér neposkytl AVC konfiguraci.");
        ByteBuffer copy = b.duplicate(); byte[] result = new byte[copy.remaining()]; copy.get(result); return result;
    }
    private void capture() {
        MediaCodec encoder = null; Surface surface = null; VirtualDisplay display = null;
        try {
            MediaFormat format = MediaFormat.createVideoFormat(MediaFormat.MIMETYPE_VIDEO_AVC, width, height);
            format.setInteger(MediaFormat.KEY_COLOR_FORMAT, MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface);
            format.setInteger(MediaFormat.KEY_BIT_RATE, bitrate);
            format.setInteger(MediaFormat.KEY_FRAME_RATE, fps);
            format.setFloat(MediaFormat.KEY_MAX_FPS_TO_ENCODER, fps);
            format.setLong(MediaFormat.KEY_REPEAT_PREVIOUS_FRAME_AFTER, 1000000L);
            format.setInteger(MediaFormat.KEY_I_FRAME_INTERVAL, 1);
            format.setInteger(MediaFormat.KEY_PROFILE, MediaCodecInfo.CodecProfileLevel.AVCProfileBaseline);
            format.setInteger(MediaFormat.KEY_MAX_B_FRAMES, 0);
            for (MediaCodecInfo info : new MediaCodecList(MediaCodecList.REGULAR_CODECS).getCodecInfos()) {
                if (!info.isEncoder() || !info.isHardwareAccelerated()) continue;
                for (String type : info.getSupportedTypes()) if (type.equalsIgnoreCase(MediaFormat.MIMETYPE_VIDEO_AVC)) {
                    MediaCodecInfo.CodecCapabilities caps = info.getCapabilitiesForType(type);
                    boolean acceptsSurface = false;
                    for (int c : caps.colorFormats) if (c == MediaCodecInfo.CodecCapabilities.COLOR_FormatSurface) acceptsSurface = true;
                    if (!acceptsSurface || !caps.isFormatSupported(format)) continue;
                    try { encoder = MediaCodec.createByCodecName(info.getName()); encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE); }
                    catch (Exception e) { if (encoder != null) encoder.release(); encoder = null; continue; }
                    break;
                }
                if (encoder != null) break;
            }
            if (encoder == null) throw new IllegalStateException("Nenalezen kompatibilní hardwarový H.264 enkodér.");
            surface = encoder.createInputSurface(); encoder.start();
            display = projection.createVirtualDisplay("QuestStream", width, height, getResources().getDisplayMetrics().densityDpi,
                DisplayManager.VIRTUAL_DISPLAY_FLAG_AUTO_MIRROR, surface, null, null);
            status = "Přenáším " + width + "×" + height + " / " + fps + " fps · " + encoder.getName();
            MediaCodec.BufferInfo info = new MediaCodec.BufferInfo(); FragmentedMp4 muxer = null; boolean haveKey = false;
            while (!destroyed && !Thread.currentThread().isInterrupted()) {
                int index = encoder.dequeueOutputBuffer(info, 100000);
                if (index == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED) {
                    if (muxer != null) throw new IllegalStateException("Změna formátu; povol nový přenos.");
                    MediaFormat actual = encoder.getOutputFormat();
                    muxer = new FragmentedMp4(actual.getInteger(MediaFormat.KEY_WIDTH), actual.getInteger(MediaFormat.KEY_HEIGHT), bytes(actual.getByteBuffer("csd-0")), bytes(actual.getByteBuffer("csd-1")), 1000000 / fps);
                    hub.initialize(muxer.initSegment());
                } else if (index >= 0) {
                    try {
                        if (info.size > 0 && (info.flags & MediaCodec.BUFFER_FLAG_CODEC_CONFIG) == 0) {
                            boolean key = (info.flags & MediaCodec.BUFFER_FLAG_KEY_FRAME) != 0;
                            if (key) haveKey = true;
                            if (muxer != null && haveKey) {
                                ByteBuffer b = encoder.getOutputBuffer(index).duplicate(); b.position(info.offset); b.limit(info.offset + info.size);
                                hub.publish(muxer.fragment(bytes(b), info.presentationTimeUs, key), key);
                                capturing = true;
                            }
                        }
                    } finally { encoder.releaseOutputBuffer(index, false); }
                }
            }
        } catch (Exception e) { if (!destroyed) status = "Přenos skončil: " + e.getMessage(); }
        finally {
            capturing = false;
            captureDone = true;
            if (display != null) display.release();
            if (encoder != null) { try { encoder.stop(); } catch (Exception ignored) {} encoder.release(); }
            if (surface != null) surface.release();
            hub.close();
            stopSelf();
        }
    }
    public void onDestroy() {
        destroyed = true;
        capturing = false;
        if (server != null) server.close();
        hub.close();
        if (projection != null) projection.stop();
        token = null;
        if (status.startsWith("Povoleno") || status.startsWith("Přenáším")) status = "Přenos je vypnutý.";
        super.onDestroy();
    }
}
