package dev.musicapp.app

import android.content.Context
import android.content.Intent
import android.webkit.JavascriptInterface
import androidx.core.content.ContextCompat

class PlaybackBridge(private val context: Context) {
  @JavascriptInterface
  fun update(title: String, artist: String, playing: Boolean) {
    val running = PlaybackService.instance
    if (running != null) {
      running.update(title, artist, playing)
      return
    }

    val intent = Intent(context, PlaybackService::class.java)
      .putExtra(PlaybackService.EXTRA_TITLE, title)
      .putExtra(PlaybackService.EXTRA_ARTIST, artist)
      .putExtra(PlaybackService.EXTRA_PLAYING, playing)
    ContextCompat.startForegroundService(context, intent)
  }
}
