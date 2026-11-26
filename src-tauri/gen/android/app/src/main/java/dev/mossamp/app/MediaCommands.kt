package dev.mossamp.app

import android.webkit.WebView
import java.lang.ref.WeakReference

/**
 * Sends playback commands from Android (media notification, lock screen,
 * Bluetooth and headset buttons, audio focus) to the app, which forwards them
 * to the Rust player. See `window.mossampMedia` in store.svelte.ts.
 */
object MediaCommands {
  private var webView: WeakReference<WebView>? = null

  fun attach(view: WebView) {
    webView = WeakReference(view)
  }

  /** [command] is one of play, pause, toggle, next, previous, seek. */
  fun send(command: String, seconds: Double = 0.0) {
    val view = webView?.get() ?: return
    val script = "window.mossampMedia && window.mossampMedia(\"$command\", $seconds)"
    view.post { view.evaluateJavascript(script, null) }
  }
}
