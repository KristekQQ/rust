package cz.kristek.questdebugawake;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Intent;
import android.os.IBinder;
import android.os.PowerManager;

public final class DebugService extends Service {
    private PowerManager.WakeLock cpu;

    @Override public void onCreate() {
        super.onCreate();
        String channel = "debug";
        getSystemService(NotificationManager.class).createNotificationChannel(
            new NotificationChannel(channel, "Wi-Fi debug", NotificationManager.IMPORTANCE_LOW));
        startForeground(1, new Notification.Builder(this, channel)
            .setSmallIcon(android.R.drawable.stat_notify_sync)
            .setContentTitle("Quest Wi-Fi debug").setContentText("CPU active for wireless debugging")
            .build());
        cpu = getSystemService(PowerManager.class).newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK, "NeonDebug:WifiSession");
        cpu.acquire();
    }

    @Override public int onStartCommand(Intent intent, int flags, int startId) { return START_STICKY; }
    @Override public IBinder onBind(Intent intent) { return null; }
    @Override public void onDestroy() {
        if (cpu != null && cpu.isHeld()) cpu.release();
        super.onDestroy();
    }
}
