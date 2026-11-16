//! Android USB bridge.
//!
//! Apps cannot enumerate or open USB devices directly on Android. The Kotlin
//! side (`UsbBridge.kt`) asks the user for permission via `UsbManager`, opens
//! the PureThermal and hands us the file descriptor of the
//! `UsbDeviceConnection`. libusb then wraps that fd (`libusb_wrap_sys_device`).

use std::sync::atomic::{AtomicI32, Ordering};

static USB_FD: AtomicI32 = AtomicI32::new(-1);

/// File descriptor of the opened PureThermal, if the Kotlin side has one.
pub fn fd() -> Option<i32> {
    let fd = USB_FD.load(Ordering::SeqCst);
    (fd >= 0).then_some(fd)
}

/// `UsbBridge.setFd(fd: Int)` (JNI, static). `-1` = device gone.
#[no_mangle]
pub extern "system" fn Java_com_xylenc4_leptongetthermal_UsbBridge_setFd(
    _env: *mut std::ffi::c_void,
    _class: *mut std::ffi::c_void,
    fd: i32,
) {
    USB_FD.store(fd, Ordering::SeqCst);
}
