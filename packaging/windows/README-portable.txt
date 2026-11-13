Lepton-GetThermal - portable Windows build (x64)
================================================

No installation needed: unzip anywhere (also a USB stick) and run
Lepton-GetThermal.exe. The app keeps its browser cache in the "webview-data"
folder next to the exe. If that folder is not writable, it uses the normal
per-user location instead.

Requirements
------------
* Windows 10 or 11, 64-bit.
* Microsoft Edge WebView2 Runtime. It comes with Windows 11 and current
  Windows 10. If the app window stays blank or does not open, install the
  "Evergreen Bootstrapper" from
  https://developer.microsoft.com/microsoft-edge/webview2/
* The Visual C++ runtime DLLs are included next to the exe.

Camera driver (one time per PC)
-------------------------------
The app talks to the PureThermal over libusb, so the camera must use the
WinUSB driver instead of the Windows webcam driver:
  1. Download Zadig from https://zadig.akeo.ie and plug in the camera.
  2. Options -> List All Devices; untick "Ignore Hubs or Composite Parents".
  3. Select "PureThermal (fw:v1.x.x) (Composite Parent)" (USB ID 1E4E 0100).
  4. Choose WinUSB and click "Replace Driver".
To undo: Device Manager -> the device -> Uninstall device, tick "Delete the
driver", then replug.

Firmware update mode (only with the USB-DFU firmware): also bind
"PureThermal DFU bootloader" (USB ID 0483 DF11) to WinUSB with Zadig once.
