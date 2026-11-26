package dev.mossamp.app

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.ServiceInfo
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.AudioManager
import android.media.MediaMetadataRetriever
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.PowerManager
import android.os.SystemClock
import android.support.v4.media.MediaMetadataCompat
import android.support.v4.media.session.MediaSessionCompat
import android.support.v4.media.session.PlaybackStateCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import androidx.media.AudioAttributesCompat
import androidx.media.AudioFocusRequestCompat
import androidx.media.AudioManagerCompat
import androidx.media.app.NotificationCompat.MediaStyle
import java.util.concurrent.Executors

/**
 * Keeps playback alive in the background and makes MossAMP a proper media app:
 * a media session (notification and lock-screen controls, Bluetooth / car /
 * headset buttons), audio focus (pauses for calls and other players) and
 * pausing when headphones or Bluetooth disconnect.
 */
class PlaybackService : Service() {
  data class State(
    val title: String,
    val artist: String,
    val playing: Boolean,
    val positionMs: Long,
    val durationMs: Long,
    val path: String,
  )

  private val main = Handler(Looper.getMainLooper())
  private val coverLoader = Executors.newSingleThreadExecutor()

  private lateinit var session: MediaSessionCompat
  private lateinit var audio: AudioManager
  private var wakeLock: PowerManager.WakeLock? = null

  private var state = State("MossAMP", "", false, 0, 0, "")
  private var cover: Bitmap? = null
  private var coverPath: String? = null
  private var started = false

  private var focusRequest: AudioFocusRequestCompat? = null
  /** Paused because of a short focus loss (e.g. a call); resume when it ends. */
  private var resumeOnFocusGain = false
  private var noisyRegistered = false

  private val noisyReceiver = object : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
      if (intent.action == AudioManager.ACTION_AUDIO_BECOMING_NOISY) {
        MediaCommands.send("pause")
      }
    }
  }

  private val focusListener = AudioManager.OnAudioFocusChangeListener { change ->
    when (change) {
      AudioManager.AUDIOFOCUS_LOSS -> {
        resumeOnFocusGain = false
        MediaCommands.send("pause")
      }
      AudioManager.AUDIOFOCUS_LOSS_TRANSIENT -> {
        resumeOnFocusGain = state.playing
        MediaCommands.send("pause")
      }
      AudioManager.AUDIOFOCUS_GAIN -> {
        if (resumeOnFocusGain) MediaCommands.send("play")
        resumeOnFocusGain = false
      }
    }
  }

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onCreate() {
    super.onCreate()
    instance = this
    audio = getSystemService(Context.AUDIO_SERVICE) as AudioManager

    getSystemService(NotificationManager::class.java).createNotificationChannel(
      NotificationChannel(CHANNEL_ID, "Playback", NotificationManager.IMPORTANCE_LOW).apply {
        setShowBadge(false)
      }
    )

    val power = getSystemService(Context.POWER_SERVICE) as PowerManager
    wakeLock = power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, "mossamp:playback").apply {
      setReferenceCounted(false)
    }

    session = MediaSessionCompat(this, "MossAMP").apply {
      setCallback(object : MediaSessionCompat.Callback() {
        override fun onPlay() = MediaCommands.send("play")
        override fun onPause() = MediaCommands.send("pause")
        override fun onSkipToNext() = MediaCommands.send("next")
        override fun onSkipToPrevious() = MediaCommands.send("previous")
        override fun onStop() = MediaCommands.send("pause")
        override fun onSeekTo(pos: Long) = MediaCommands.send("seek", pos / 1000.0)
      })
      setSessionActivity(openAppIntent())
      isActive = true
    }
  }

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    when (intent?.action) {
      ACTION_PLAY_PAUSE -> MediaCommands.send(if (state.playing) "pause" else "play")
      ACTION_NEXT -> MediaCommands.send("next")
      ACTION_PREVIOUS -> MediaCommands.send("previous")
    }

    if (!started) {
      started = true
      pending?.let { state = it }
      pending = null
      // The three-argument startForeground() only exists from Android 10 (API 29);
      // ServiceCompat drops the type on older versions instead of crashing.
      ServiceCompat.startForeground(
        this,
        NOTIFICATION_ID,
        buildNotification(),
        ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PLAYBACK,
      )
      apply(state, force = true)
    }
    return START_NOT_STICKY
  }

  /** Called from the JavaScript bridge thread. */
  fun post(next: State) {
    main.post { apply(next) }
  }

  private fun apply(next: State, force: Boolean = false) {
    val previous = state
    state = next

    if (force || next.path != previous.path || next.title != previous.title || next.durationMs != previous.durationMs) {
      updateMetadata()
      loadCover(next.path)
    }
    if (force || next.playing != previous.playing) {
      if (next.playing) onStartedPlaying() else onStoppedPlaying()
    }
    updatePlaybackState()
    getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, buildNotification())
  }

  private fun onStartedPlaying() {
    wakeLock?.acquire()
    requestFocus()
    if (!noisyRegistered) {
      ContextCompat.registerReceiver(
        this,
        noisyReceiver,
        IntentFilter(AudioManager.ACTION_AUDIO_BECOMING_NOISY),
        ContextCompat.RECEIVER_NOT_EXPORTED,
      )
      noisyRegistered = true
    }
  }

  private fun onStoppedPlaying() {
    if (wakeLock?.isHeld == true) wakeLock?.release()
    if (noisyRegistered) {
      unregisterReceiver(noisyReceiver)
      noisyRegistered = false
    }
    // Focus is kept while paused so a short interruption can resume us;
    // it is given up when something else takes it for good.
  }

  private fun requestFocus() {
    val request = focusRequest ?: AudioFocusRequestCompat.Builder(AudioManagerCompat.AUDIOFOCUS_GAIN)
      .setAudioAttributes(
        AudioAttributesCompat.Builder()
          .setUsage(AudioAttributesCompat.USAGE_MEDIA)
          .setContentType(AudioAttributesCompat.CONTENT_TYPE_MUSIC)
          .build()
      )
      .setOnAudioFocusChangeListener(focusListener, main)
      // Let Android lower our volume for navigation prompts etc. by itself.
      .setWillPauseWhenDucked(false)
      .build()
      .also { focusRequest = it }
    AudioManagerCompat.requestAudioFocus(audio, request)
  }

  private fun updateMetadata() {
    session.setMetadata(
      MediaMetadataCompat.Builder()
        .putString(MediaMetadataCompat.METADATA_KEY_TITLE, state.title)
        .putString(MediaMetadataCompat.METADATA_KEY_ARTIST, state.artist)
        .putLong(MediaMetadataCompat.METADATA_KEY_DURATION, state.durationMs)
        .putBitmap(MediaMetadataCompat.METADATA_KEY_ALBUM_ART, cover)
        .build()
    )
  }

  private fun updatePlaybackState() {
    val actions = PlaybackStateCompat.ACTION_PLAY or
      PlaybackStateCompat.ACTION_PAUSE or
      PlaybackStateCompat.ACTION_PLAY_PAUSE or
      PlaybackStateCompat.ACTION_SKIP_TO_NEXT or
      PlaybackStateCompat.ACTION_SKIP_TO_PREVIOUS or
      PlaybackStateCompat.ACTION_SEEK_TO or
      PlaybackStateCompat.ACTION_STOP
    session.setPlaybackState(
      PlaybackStateCompat.Builder()
        .setActions(actions)
        .setState(
          if (state.playing) PlaybackStateCompat.STATE_PLAYING else PlaybackStateCompat.STATE_PAUSED,
          state.positionMs,
          if (state.playing) 1f else 0f,
          SystemClock.elapsedRealtime(),
        )
        .build()
    )
  }

  /** Reads the embedded cover off the main thread. */
  private fun loadCover(path: String) {
    if (path == coverPath) return
    coverPath = path
    cover = null
    if (path.isEmpty()) return
    coverLoader.execute {
      val bitmap = try {
        MediaMetadataRetriever().run {
          try {
            setDataSource(path)
            embeddedPicture?.let { decodeCover(it) }
          } finally {
            release()
          }
        }
      } catch (e: Exception) {
        null
      }
      main.post {
        if (coverPath == path) {
          cover = bitmap
          updateMetadata()
          getSystemService(NotificationManager::class.java).notify(NOTIFICATION_ID, buildNotification())
        }
      }
    }
  }

  private fun decodeCover(bytes: ByteArray): Bitmap? {
    // Cap the size: covers can be several megapixels, the controls need ~512px.
    val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
    var sample = 1
    while (bounds.outWidth / (sample * 2) >= 512 && bounds.outHeight / (sample * 2) >= 512) sample *= 2
    return BitmapFactory.decodeByteArray(bytes, 0, bytes.size, BitmapFactory.Options().apply { inSampleSize = sample })
  }

  private fun openAppIntent(): PendingIntent = PendingIntent.getActivity(
    this,
    0,
    Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
    PendingIntent.FLAG_IMMUTABLE,
  )

  private fun action(action: String, code: Int): PendingIntent = PendingIntent.getService(
    this,
    code,
    Intent(this, PlaybackService::class.java).setAction(action),
    PendingIntent.FLAG_IMMUTABLE,
  )

  private fun buildNotification(): Notification {
    val playPause = if (state.playing) {
      NotificationCompat.Action(android.R.drawable.ic_media_pause, "Pause", action(ACTION_PLAY_PAUSE, 1))
    } else {
      NotificationCompat.Action(android.R.drawable.ic_media_play, "Play", action(ACTION_PLAY_PAUSE, 1))
    }

    return NotificationCompat.Builder(this, CHANNEL_ID)
      .setSmallIcon(android.R.drawable.ic_media_play)
      .setContentTitle(state.title)
      .setContentText(state.artist)
      .setLargeIcon(cover)
      .setContentIntent(openAppIntent())
      .addAction(android.R.drawable.ic_media_previous, "Previous", action(ACTION_PREVIOUS, 2))
      .addAction(playPause)
      .addAction(android.R.drawable.ic_media_next, "Next", action(ACTION_NEXT, 3))
      .setStyle(
        MediaStyle()
          .setMediaSession(session.sessionToken)
          .setShowActionsInCompactView(0, 1, 2)
      )
      .setOngoing(state.playing)
      .setOnlyAlertOnce(true)
      .setShowWhen(false)
      .setCategory(NotificationCompat.CATEGORY_TRANSPORT)
      .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
      .build()
  }

  override fun onDestroy() {
    if (wakeLock?.isHeld == true) wakeLock?.release()
    if (noisyRegistered) unregisterReceiver(noisyReceiver)
    focusRequest?.let { AudioManagerCompat.abandonAudioFocusRequest(audio, it) }
    session.release()
    coverLoader.shutdown()
    instance = null
    super.onDestroy()
  }

  companion object {
    const val CHANNEL_ID = "playback"
    const val NOTIFICATION_ID = 1
    const val ACTION_PLAY_PAUSE = "dev.mossamp.app.PLAY_PAUSE"
    const val ACTION_NEXT = "dev.mossamp.app.NEXT"
    const val ACTION_PREVIOUS = "dev.mossamp.app.PREVIOUS"

    var instance: PlaybackService? = null
    /** State to show once the service has started. */
    @Volatile var pending: State? = null
  }
}
