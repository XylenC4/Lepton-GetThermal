package com.xylenc4.leptongetthermal

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.hardware.usb.UsbDevice
import android.hardware.usb.UsbDeviceConnection
import android.hardware.usb.UsbManager
import android.os.Build
import android.util.Log

/**
 * Finds the PureThermal (VID 0x1E4E / PID 0x0100), asks for USB permission,
 * opens it and passes the file descriptor to Rust (libusb_wrap_sys_device).
 */
object UsbBridge {
    private const val TAG = "LeptonUsb"
    private const val VID = 0x1E4E
    private const val PID = 0x0100
    private const val ACTION_PERMISSION = "com.xylenc4.leptongetthermal.USB_PERMISSION"

    init {
        System.loadLibrary("lepton_getthermal_lib")
    }

    /** Implemented in Rust (src/android_usb.rs). -1 = no device. */
    @JvmStatic
    external fun setFd(fd: Int)

    private var connection: UsbDeviceConnection? = null
    private var device: UsbDevice? = null
    private var permissionPending = false
    private var registered = false

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            when (intent.action) {
                ACTION_PERMISSION -> {
                    permissionPending = false
                    if (intent.getBooleanExtra(UsbManager.EXTRA_PERMISSION_GRANTED, false)) {
                        scan(context)
                    } else {
                        Log.w(TAG, "USB permission denied")
                    }
                }
                UsbManager.ACTION_USB_DEVICE_ATTACHED -> scan(context)
                UsbManager.ACTION_USB_DEVICE_DETACHED -> {
                    val d = usbDevice(intent)
                    if (d == null || d.deviceName == device?.deviceName) close()
                }
            }
        }
    }

    @Suppress("DEPRECATION")
    private fun usbDevice(intent: Intent): UsbDevice? =
        if (Build.VERSION.SDK_INT >= 33)
            intent.getParcelableExtra(UsbManager.EXTRA_DEVICE, UsbDevice::class.java)
        else intent.getParcelableExtra(UsbManager.EXTRA_DEVICE)

    fun start(context: Context) {
        if (!registered) {
            val filter = IntentFilter().apply {
                addAction(ACTION_PERMISSION)
                addAction(UsbManager.ACTION_USB_DEVICE_ATTACHED)
                addAction(UsbManager.ACTION_USB_DEVICE_DETACHED)
            }
            if (Build.VERSION.SDK_INT >= 33) {
                context.registerReceiver(receiver, filter, Context.RECEIVER_EXPORTED)
            } else {
                context.registerReceiver(receiver, filter)
            }
            registered = true
        }
        scan(context)
    }

    @Synchronized
    fun scan(context: Context) {
        val mgr = context.getSystemService(Context.USB_SERVICE) as UsbManager
        val dev = mgr.deviceList.values.firstOrNull { it.vendorId == VID && it.productId == PID }
        if (dev == null) {
            close()
            return
        }
        if (connection != null && device?.deviceName == dev.deviceName) return
        close()
        if (!mgr.hasPermission(dev)) {
            if (permissionPending) return
            permissionPending = true
            val flags = if (Build.VERSION.SDK_INT >= 31) PendingIntent.FLAG_MUTABLE else 0
            val intent = Intent(ACTION_PERMISSION).setPackage(context.packageName)
            mgr.requestPermission(dev, PendingIntent.getBroadcast(context, 0, intent, flags))
            return
        }
        val conn = mgr.openDevice(dev)
        if (conn == null) {
            Log.e(TAG, "openDevice failed")
            return
        }
        // Detach any kernel driver (e.g. uvcvideo) so libusb can claim the interfaces.
        for (i in 0 until dev.interfaceCount) {
            val itf = dev.getInterface(i)
            if (conn.claimInterface(itf, true)) conn.releaseInterface(itf)
        }
        connection = conn
        device = dev
        Log.i(TAG, "PureThermal opened, fd=${conn.fileDescriptor}")
        setFd(conn.fileDescriptor)
    }

    @Synchronized
    fun close() {
        if (connection == null) return
        setFd(-1)
        connection?.close()
        connection = null
        device = null
        Log.i(TAG, "PureThermal closed")
    }
}
