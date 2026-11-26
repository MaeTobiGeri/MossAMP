package dev.mossamp.app

import android.content.Context
import android.content.Intent
import android.webkit.JavascriptInterface
import androidx.core.content.ContextCompat

/** Called from the web app (window.MossampAndroid) whenever playback changes. */
class PlaybackBridge(private val context: Context) {
  @JavascriptInterface
  fun update(
    title: String,
    artist: String,
    playing: Boolean,
    positionSeconds: Double,
    durationSeconds: Double,
    path: String,
  ) {
    val state = PlaybackService.State(
      title = title,
      artist = artist,
      playing = playing,
      positionMs = (positionSeconds * 1000).toLong(),
      durationMs = (durationSeconds * 1000).toLong(),
      path = path,
    )

    val running = PlaybackService.instance
    if (running != null) {
      running.post(state)
      return
    }

    PlaybackService.pending = state
    ContextCompat.startForegroundService(context, Intent(context, PlaybackService::class.java))
  }
}
