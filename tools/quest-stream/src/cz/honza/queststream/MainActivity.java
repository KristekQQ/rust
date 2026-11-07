package cz.honza.queststream;

import android.app.*;
import android.os.*;
import android.content.*;
import android.media.projection.MediaProjectionManager;
import android.widget.*;
import android.view.View;
import java.net.*;
import java.util.*;

public final class MainActivity extends Activity {
    private TextView status;
    private Spinner profile;
    private String token;
    private int selectedProfile;
    private final Handler handler = new Handler();
    private final Runnable refresh = new Runnable() {
        public void run() {
            StringBuilder s = new StringBuilder(StreamService.status);
            if (StreamService.token != null) {
                s.append("\n\nNa počítači ve stejné Wi-Fi otevři:\n");
                try {
                    for (NetworkInterface nic : Collections.list(NetworkInterface.getNetworkInterfaces())) {
                        if (!nic.isUp() || nic.isLoopback()) continue;
                        for (InetAddress a : Collections.list(nic.getInetAddresses()))
                            if (a instanceof Inet4Address && !a.isLoopbackAddress() && !a.isLinkLocalAddress())
                                s.append("http://").append(a.getHostAddress()).append(":8088/?token=").append(StreamService.token).append("\n");
                    }
                } catch (Exception e) { s.append("IP adresu se nepodařilo zjistit."); }
            }
            status.setText(s.toString());
            handler.postDelayed(this, 1500);
        }
    };
    public void onCreate(Bundle state) {
        super.onCreate(state);
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL); root.setPadding(40, 32, 40, 24);
        TextView title = new TextView(this); title.setText("Quest Stream"); title.setTextSize(28); root.addView(title);
        TextView help = new TextView(this);
        help.setText("Živý obraz do prohlížeče přes místní síť.\nPovol sdílení celé obrazovky a pak otevři VR aplikaci.\nBez zvuku. Nejvýše dvě okna současně. Odkaz umožňuje sledování; sdílej ho jen s důvěryhodnými lidmi.");
        root.addView(help);
        profile = new Spinner(this);
        profile.setAdapter(new ArrayAdapter<String>(this, android.R.layout.simple_spinner_dropdown_item,
            new String[]{"720p · 30 fps · 3 Mb/s", "Úsporný · 540p · 24 fps · 2 Mb/s"}));
        root.addView(profile);
        Button start = new Button(this); start.setText("Povolit přenos"); root.addView(start);
        start.setOnClickListener(v -> {
            if (StreamService.token != null) {
                Toast.makeText(this, "Nejdřív ukonči současný přenos.", Toast.LENGTH_SHORT).show(); return;
            }
            selectedProfile = profile.getSelectedItemPosition();
            token = UUID.randomUUID().toString().replace("-", "");
            MediaProjectionManager mgr = getSystemService(MediaProjectionManager.class);
            startActivityForResult(mgr.createScreenCaptureIntent(), 17);
        });
        Button stop = new Button(this); stop.setText("Ukončit přenos"); root.addView(stop);
        stop.setOnClickListener(v -> stopService(new Intent(this, StreamService.class)));
        status = new TextView(this); status.setTextSize(18); status.setTextIsSelectable(true); root.addView(status);
        ScrollView scroll = new ScrollView(this); scroll.addView(root); setContentView(scroll);
        if (state != null) { token = state.getString("token"); selectedProfile = state.getInt("profile"); }
    }
    protected void onSaveInstanceState(Bundle b) { super.onSaveInstanceState(b); b.putString("token",token); b.putInt("profile", selectedProfile); }
    protected void onActivityResult(int request, int result, Intent data) {
        super.onActivityResult(request, result, data);
        if (request == 17 && result == RESULT_OK && data != null && token != null) {
            Intent service = new Intent(this, StreamService.class);
            service.putExtra("result", result).putExtra("data", data).putExtra("token", token).putExtra("profile", selectedProfile);
            startForegroundService(service);
        }
    }
    protected void onResume() { super.onResume(); handler.post(refresh); }
    protected void onPause() { handler.removeCallbacks(refresh); super.onPause(); }
}
