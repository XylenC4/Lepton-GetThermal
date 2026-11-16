# Lepton-GetThermal for Android

Android port of [Lepton-GetThermal](https://github.com/XylenC4/Lepton-GetThermal): a FLIR Lepton
viewer for the GroupGets **PureThermal** USB board. It runs on phones and tablets that support
USB host (OTG).

## Install

1. Download `Lepton-GetThermal-Android-<version>.apk` from the Releases page.
2. Allow installing from unknown sources, then open the APK.
3. Plug in the PureThermal with a USB-C/OTG adapter. Android asks whether to open
   Lepton-GetThermal and whether to allow USB access. Tap **OK**. Tick "always" to skip the
   question next time.

The app connects automatically once access is granted, the same way the desktop app does.

## How it works

| Layer | File |
|---|---|
| USB permission, open device, hand fd to Rust | `src-tauri/gen/android/app/src/main/java/com/xylenc4/leptongetthermal/UsbBridge.kt` |
| Auto-launch on plug-in (VID `1E4E` / PID `0100`) | `AndroidManifest.xml` + `res/xml/device_filter.xml` |
| JNI entry `UsbBridge.setFd(fd)` | `src-tauri/src/android_usb.rs` |
| libusb on the fd (`libusb_wrap_sys_device`) | `UsbStream::open()` → `open_fd()` in `src-tauri/src/usb_stream_libusb.rs` |

Android apps cannot enumerate USB devices themselves. `UsbManager` opens the device, and its file
descriptor is wrapped by the vendored libusb. Everything after that (UVC negotiation,
isochronous streaming, Lepton controls) is the shared Rust code.

## Differences from desktop

- **AI upscaling is off.** ONNX Runtime ships no Android build for the `ort` crate, so
  `processing/upscale_stub.rs` replaces it. Turning the upscale toggle on does nothing.
- **PNG/CSV export** uses `<a download>`, which the Android WebView ignores. Not ported yet.
- In portrait, the layout stacks video, readouts and controls vertically. Landscape matches desktop.

## Build

Prerequisites: Rust with the `aarch64-linux-android`, `armv7-linux-androideabi` and
`x86_64-linux-android` targets, Node.js 20 or newer, JDK 17, Android SDK (platform 36), NDK 27.

```bash
export JAVA_HOME=/path/to/jdk-17 ANDROID_HOME=~/Android/Sdk NDK_HOME=$ANDROID_HOME/ndk/27.2.12479018
npm ci
npx tauri android build --apk --target aarch64 --target armv7 --target x86_64
# -> src-tauri/gen/android/app/build/outputs/apk/universal/release/app-universal-release.apk
```

Release signing reads `src-tauri/gen/android/key.properties` (gitignored):

```properties
storeFile=/abs/path/release.jks
storePassword=...
keyAlias=...
keyPassword=...
```

Debugging: `npx tauri android dev`, or `adb logcat -s LeptonUsb` for the USB bridge.
