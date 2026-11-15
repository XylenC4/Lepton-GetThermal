# Lepton-GetThermal: Windows / Linux (and later Android)

The original app only supported macOS, using IOKit. This branch adds a **libusb backend**
(`src-tauri/src/usb_stream_libusb.rs`) that is used on every non-macOS platform.
It has the same `UsbStream` API, so the rest of the app is unchanged.

| Platform | USB backend | Status |
|---|---|---|
| macOS | IOKit (`usb_stream_iokit.rs`, unchanged) | as upstream |
| Linux | libusb (vendored 1.0.27) | tested with PureThermal fw 1.3.0 + Lepton 3.5 |
| Windows x64 | libusb, needs WinUSB driver (Zadig) | tested (Windows 11, Lepton 3.5); native or cross-compiled build |
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

Troubleshooting: errors and panics are written to `lepton-getthermal.log` next to the exe (or
in `%TEMP%` if that folder isn't writable). Right-click → *Inspect* opens the developer tools,
also in release builds. If the camera still uses the webcam driver, the app says so and shows
the Zadig steps above instead of failing to connect.

### Building on Windows (native)

Prerequisites:
- Visual Studio 2022 Build Tools, with the *Desktop development with C++* workload and a
  Windows 11 SDK;
- [rustup](https://rustup.rs), with the `stable-x86_64-pc-windows-msvc` toolchain;
- Node.js LTS.

Then:

```powershell
npm ci
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1              # portable zip
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Installer   # + NSIS setup.exe
powershell -ExecutionPolicy Bypass -File scripts\build-windows.ps1 -Debug       # debug exe only
```

The zip is written to `src-tauri\target\release\portable\`. The script stages the runtime DLLs
in `src-tauri\windows-libs\`.

On machines with little memory, especially VMs without a pagefile, parallel `rustc` jobs run
out of memory. The failures look like compiler bugs: ICEs, "import resolution is stuck",
0xc0000409. The script therefore caps `CARGO_BUILD_JOBS` at 1 job per 4 GB of commit limit;
set the variable yourself to override this.

### Building on Linux (cross-compile for Windows)

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

## Viewer features added in this fork

All platforms. The left panel has three tabs: **View** (palette, polarity, contrast, rotation,
markers, upscale, capture), **Analyze** (peak hold, difference, isotherm, measure) and
**Device** (info, measurement settings, FFC, theme).

- **Box and line measurements** (*Analyze → Measure*): *+ Box* or *+ Line*, then drag on the
  image (Esc cancels). Up to 6 tools, each with a color and a label (B1, L1, …) on the image.
  - The table shows each tool's live **min / max / avg** in °C. Click a row to select it.
  - The **graph** below the image shows the trend of all tools over the last 1, 5 or 15 min
    (*Max / Avg / Min*), and for a selected line its **temperature profile** along the line.
    *Hide graph / Show graph* toggles it.
  - *CSV* exports the recorded trend (time, then min/max/avg per tool). *Clear* removes all
    tools and their history.
- **Measurement settings** (*Device → Measurement*), applied in the Lepton:
  - **Gain:** *High* (≈ −10…140 °C, most sensitive, default), *Low* (≈ −10…400 °C) or *Auto*.
    Changing gain runs an FFC.
  - **Emissivity** with material presets (skin 0.98, matte black paint 0.97, wood/plastic 0.90,
    oxidized steel 0.80, polished metal 0.10, …) or a custom value 0.10–1.00.
  - **Reflected temperature** (−20…60 °C), which matters most for low emissivities.

- **Equalized contrast** (*View → Contrast: Equalized*): plateau histogram equalization
  spreads the colors over the temperatures that actually occur, so one hot object no longer
  flattens the rest of the scene. The scale stays a temperature axis with the range handles;
  its gradient is drawn non-linearly, from 17 reference points computed in Rust.
- **Temperature under the pointer:** hover anywhere over the image to read the temperature and
  the pixel position, taken from the raw frame on screen.
- **Peak hold** (*Analyze → Peak hold: Max / Min*): each pixel keeps its hottest (or coldest)
  value since the start or the last *Reset*. This shows intermittent hot spots. The scale,
  markers and pointer readout use the held image; the spot temperature stays live.
- **Themes:** *Auto / Light / Dark* switch at the bottom of the left panel. *Auto* follows the
  OS theme; the choice is remembered, and the window title bar follows it.
- **Auto-connect:** no Connect button. The app watches for the PureThermal (every 1.5 s, without
  opening it) and connects as soon as it is plugged in, also after it was unplugged during a
  session. Until then a status screen says what is going on: no camera, connecting, wrong
  Windows driver (with the Zadig fix), or a failed connect (retried every few seconds, *Try now*
  to retry at once). macOS can't check presence without opening the device, so it simply tries
  to connect every few seconds.
- **Rotation** (*View → Rotation: 0° / 90° / 180° / 270°*, clockwise): the frame is rotated in
  the backend, so the pointer readout, boxes and lines, min/max markers, spot meter and PNG
  capture all follow it. Existing boxes and lines turn with the image.
- **Session file:** all settings (palette, polarity, contrast, rotation, range, isotherm,
  difference settings, gain/emissivity, boxes and lines, graph, spot meter, theme, tab) are saved
  to `session.json` and restored at the next start. It sits next to the executable when that
  folder is writable (portable use), otherwise in the per-user app config folder
  (`%APPDATA%\<app identifier>` on Windows). Delete it to start with defaults. Not saved: the
  difference reference and the trend history.
- **Temperature range on the scale:** drag the two handles on the color scale (right panel) to
  fix the range. Pixels outside it clip to the end colors, and dashed marks show the scene's
  real min/max. Clicking the bar moves the nearest handle; arrow keys move a selected handle
  0.1 °C, or 1 °C with Shift. Double-click the scale or press *Auto* to go back to the
  automatic range.
- **Difference mode** (*Difference* section in the left panel): *Record reference* stores each
  pixel's **minimum and maximum** over 16 frames (~2 s); that range is the pixel's own noise.
  - With *Show only changes* on, only pixels that leave that range are colored: warmer in
    red → yellow, colder in blue → cyan. The rest of the scene stays as a dimmed grayscale.
  - The scale on the right then has three handles: full color for warming, full color for
    cooling, and the extra noise margin.
  - The comparison runs on the raw radiometric data in Rust (`processing/diff.rs`). The spot
    temperature and the min/max values stay real temperatures.

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
