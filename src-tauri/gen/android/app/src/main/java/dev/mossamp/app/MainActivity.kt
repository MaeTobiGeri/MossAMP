package dev.mossamp.app

import android.Manifest
import android.os.Build
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.app.ActivityCompat
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    val root = findViewById<View>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(root) { view, insets ->
      val bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout())
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      insets
    }

    requestRuntimePermissions()
  }

  override fun onWebViewCreate(webView: WebView) {
    webView.addJavascriptInterface(PlaybackBridge(this), "MossampAndroid")
  }

  private fun requestRuntimePermissions() {
    val permissions = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      arrayOf(Manifest.permission.POST_NOTIFICATIONS, Manifest.permission.READ_MEDIA_AUDIO)
    } else {
      arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE)
    }
    ActivityCompat.requestPermissions(this, permissions, 0)
  }
}
