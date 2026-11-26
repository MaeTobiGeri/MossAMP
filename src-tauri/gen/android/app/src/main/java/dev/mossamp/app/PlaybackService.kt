package dev.mossamp.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat

class PlaybackService : Service() {
  private var wakeLock: PowerManager.WakeLock? = null

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    instance = this
    val manager = getSystemService(NotificationManager::class.java)
    manager.createNotificationChannel(
      NotificationChannel(CHANNEL_ID, "Playback", NotificationManager.IMPORTANCE_LOW)
    )

    val power = getSystemService(Context.POWER_SERVICE) as PowerManager
    wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mossamp:playback").apply {
      setReferenceCounted(false)
      acquire()
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    val notification = buildNotification(
      intent?.getStringExtra(EXTRA_TITLE) ?: "MossAMP",
      intent?.getStringExtra(EXTRA_ARTIST) ?: "",
      intent?.getBooleanExtra(EXTRA_PLAYING, false) ?: false,
    )
    startForeground(NOTIFICATION_ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK)
    return START_NOT_STICKY
  }

  fun update(title: String, artist: String, playing: Boolean) {
    val manager = getSystemService(NotificationManager::class.java)
    manager.notify(NOTIFICATION_ID, buildNotification(title, artist, playing))
  }

  private fun buildNotification(title: String, artist: String, playing: Boolean): Notification {
    val open = PendingIntent.getActivity(
      this,
      0,
      Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
      PendingIntent.FLAG_IMMUTABLE,
    )

    return NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(if (playing) android.R.drawable.ic_media_play else android.R.drawable.ic_media_pause)
      .setContentTitle(title)
      .setContentText(artist)
      .setContentIntent(open)
      .setOngoing(playing)
      .setOnlyAlertOnce(true)
      .setCategory(NotificationCompat.CATEGORY_TRANSPORT)
      .build()
  }

  override fun onDestroy() {
    wakeLock?.release()
    instance = null
    super.onDestroy()
  }

  companion object {
    const val CHANNEL_ID = "playback"
    const val NOTIFICATION_ID = 1
    const val EXTRA_TITLE = "title"
    const val EXTRA_ARTIST = "artist"
    const val EXTRA_PLAYING = "playing"

    var instance: PlaybackService? = null
  }
}
