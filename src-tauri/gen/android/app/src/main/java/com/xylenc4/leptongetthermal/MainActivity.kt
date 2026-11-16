package com.xylenc4.leptongetthermal

import android.content.Intent
import android.os.Bundle
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
    UsbBridge.start(applicationContext)
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    // Launched (or brought to front) by plugging in the PureThermal.
    UsbBridge.scan(applicationContext)
  }

  override fun onResume() {
    super.onResume()
    UsbBridge.scan(applicationContext)
  }
}
