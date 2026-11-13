# Lepton-GetThermal: Windows / Linux (and later Android)

The original app only supported macOS, using IOKit. This branch adds a **libusb backend**
(`src-tauri/src/usb_stream_libusb.rs`) that is used on every non-macOS platform.
It has the same `UsbStream` API, so the rest of the app is unchanged.

| Platform | USB backend | Status |
|---|---|---|
| macOS | IOKit (`usb_stream_iokit.rs`, unchanged) | as upstream |
| Linux | libusb (vendored 1.0.27) | tested with PureThermal fw 1.3.0 + Lepton 3.5 |
| Windows x64 | libusb, needs WinUSB driver (Zadig) | cross-compiled from Linux |
| Android | libusb via `UsbStream::open_fd()` (`libusb_wrap_sys_device`) | prepared, not built yet |

## Windows

### 1. Install the WinUSB driver (one time, per PC)

libusb cannot talk to a device while Windows' webcam driver (`usbvideo.sys`) owns it.

1. Download Zadig from https://zadig.akeo.ie and plug in the PureThermal.
2. **Options → List All Devices**, then **untick "Ignore Hubs or Composite Parents"**.
3. Select **`PureThermal (fw:v1.x.x) (Composite Parent)`**. Check that the USB ID is `1E4E 0100`.
4. Pick **WinUSB** as the target driver and click **Replace Driver**.

The camera then no longer shows up as a webcam in other apps. To undo this, open Device Manager,
right-click the device, choose *Uninstall device* and tick *Delete the driver*, then replug it.

### 2. Run the app (portable, nothing to install)

Unzip `Lepton-GetThermal_<version>_x64-portable.zip` anywhere (a USB stick works too) and start
`Lepton-GetThermal.exe`. The zip contains the exe, `DirectML.dll` and the Visual C++ runtime DLLs,
which Microsoft allows to be shipped alongside an app.
The WebView2 cache goes to `webview-data/` next to the exe if that folder is writable.
WebView2 itself comes with Windows 11 and current Windows 10.

### Building on Linux

```bash
sudo apt install clang lld llvm cabextract          # + nsis for --installer
rustup target add x86_64-pc-windows-msvc
cargo install --locked cargo-xwin
npm install
scripts/build-windows.sh               # -> src-tauri/target/x86_64-pc-windows-msvc/release/portable/*.zip
scripts/build-windows.sh --installer   # also builds an NSIS setup.exe
```

## Linux

```bash
sudo apt install libwebkit2gtk-4.1-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
sudo cp packaging/linux/99-purethermal.rules /etc/udev/rules.d/ && sudo udevadm control --reload
# replug the camera
npm install && npm run tauri build -- --bundles deb,appimage
```

The kernel `uvcvideo` driver is detached automatically while the app streams, and re-attached on close.

Hardware test (camera attached):

```bash
cd src-tauri && cargo test --lib hw_ -- --ignored --nocapture
```

## Fixes that also affect macOS

The PureThermal forwards the host's request length to the Lepton, and the Lepton rejects any
length that doesn't match the attribute size. Upstream read 32-bit SDK enums as a single 16-bit
word, and the 32-byte FFC shutter-mode object as one word. As a result these calls silently failed:
AGC off, auto-FFC off, polarity, LUT, gain mode and TLinear resolution. The last one made
`supports_radiometry()` always return false, which broke spot temperatures. The fix is in
`camera/lepton.rs` (`get_enum` / `set_enum`, plus read-modify-write of the FFC object).

## Android (planned)

* A Kotlin Tauri plugin finds `1e4e:0100` through `UsbManager`, requests permission, opens the device
  and passes `connection.fileDescriptor` to Rust, which calls `UsbStream::open_fd(fd)`.
* The ONNX upscaler (`ort`) should be feature-gated off on Android at first.

## PureThermal firmware

The camera reports `PureThermal (fw:v1.3.0)`. The latest release is **1.3.1** (2026-09), which fixes
stalls when a UVC stream is restarted and adds Lepton /CS resync and hardware-reset recovery.
The firmware has no software command to enter the bootloader, so a USB update needs the
STM32 ROM DFU mode:

1. Hold **BOOT**, press and release **RST**, then release BOOT. The LED dims, and `lsusb` shows `0483:df11`.
   The original PureThermal 1 has no BOOT button; some clones do. Without one, use an ST-Link on SWD.
2. Back up the current firmware first:
   `dfu-util -a 0 -d 0483:df11 -s 0x08000000:0x80000 -U pt1-backup.bin`
3. Flash: `dfu-util -a 0 -d 0483:df11 -s 0x08000000:leave -D pt1-v1.3.1.bin`

ST-Link alternative: `st-flash read pt1-backup.bin 0x08000000 0x80000`, then
`st-flash --reset write pt1-v1.3.1.bin 0x08000000`.
