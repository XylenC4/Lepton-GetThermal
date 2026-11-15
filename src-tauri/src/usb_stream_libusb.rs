//! libusb backend for PureThermal Y16 video capture (Linux, Windows, Android).
//!
//! Same public API as the macOS IOKit backend (`usb_stream_iokit.rs`):
//! `open()`, `start_stream()`, `stop_stream()`, `get_ctrl()`, `set_ctrl()`.
//!
//! Platform notes:
//! - Linux: the kernel `uvcvideo` driver is detached automatically while the
//!   app owns the device; a udev rule is needed for non-root access.
//! - Windows: the device must be bound to WinUSB or libusbK (e.g. via Zadig),
//!   libusb cannot talk to devices owned by `usbvideo.sys`.
//! - Android: the Java side obtains a file descriptor from `UsbManager` and
//!   passes it to [`UsbStream::open_fd`] (uses `libusb_wrap_sys_device`).

use std::os::raw::{c_int, c_void};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use libusb1_sys as ffi;
use libusb1_sys::constants::*;
use parking_lot::Mutex;

use crate::camera::types::CameraError;
use crate::uvc_descriptors::{self, UvcStreamConfig};
use crate::uvc_payload::FrameAssembler;

const PT_VID: u16 = 0x1e4e;
const PT_PID: u16 = 0x0100;

/// Isochronous packets per transfer (full-speed: 1 packet per 1 ms frame).
const ISO_PACKETS_PER_TRANSFER: usize = 32;
/// Transfers kept in flight.
const NUM_TRANSFERS: usize = 4;
/// Control transfer timeout.
const CTRL_TIMEOUT_MS: u32 = 1000;

const UVC_SET_CUR: u8 = 0x01;
const UVC_GET_CUR: u8 = 0x81;
const VS_PROBE_CONTROL: u16 = 0x01;
const VS_COMMIT_CONTROL: u16 = 0x02;
const REQ_CLASS_IFACE_OUT: u8 = 0x21;
const REQ_CLASS_IFACE_IN: u8 = 0xA1;

fn err_name(code: c_int) -> String {
    unsafe {
        let p = ffi::libusb_error_name(code);
        if p.is_null() {
            return format!("libusb error {code}");
        }
        std::ffi::CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

// ---------------------------------------------------------------------------
// UVC Probe Control structure (26 bytes, UVC 1.0)
// ---------------------------------------------------------------------------

#[repr(C, packed)]
#[derive(Clone, Copy, Default)]
struct UvcProbeControl {
    bm_hint: u16,
    b_format_index: u8,
    b_frame_index: u8,
    dw_frame_interval: u32,
    w_key_frame_rate: u16,
    w_p_frame_rate: u16,
    w_comp_quality: u16,
    w_comp_window_size: u16,
    w_delay: u16,
    dw_max_video_frame_size: u32,
    dw_max_payload_transfer_size: u32,
}
const _: () = assert!(std::mem::size_of::<UvcProbeControl>() == 26);

// ---------------------------------------------------------------------------
// Streaming state
// ---------------------------------------------------------------------------

struct StreamState {
    assembler: Mutex<FrameAssembler>,
    callback: Box<dyn Fn(&[u8], u32, u32) + Send + Sync>,
    width: u32,
    height: u32,
    packet_len: usize,
    running: AtomicBool,
    /// Transfers currently submitted to libusb.
    in_flight: AtomicUsize,
}

/// Per-transfer context (user_data of the libusb transfer).
struct TransferCtx {
    state: Arc<StreamState>,
    _buf: Vec<u8>,
}

struct Transfers(Vec<*mut ffi::libusb_transfer>);
// SAFETY: transfers are only touched by the event thread (callbacks) and by
// stop_stream() after the event thread has been joined.
unsafe impl Send for Transfers {}

struct ActiveStream {
    state: Arc<StreamState>,
    transfers: Transfers,
    event_thread: thread::JoinHandle<()>,
}

// ---------------------------------------------------------------------------
// UsbStream
// ---------------------------------------------------------------------------

pub struct UsbStream {
    ctx: *mut ffi::libusb_context,
    handle: *mut ffi::libusb_device_handle,
    config: UvcStreamConfig,
    vc_iface: u8,
    claimed: Vec<u8>,
    active: Mutex<Option<ActiveStream>>,
}

// SAFETY: libusb contexts and handles are thread-safe.
unsafe impl Send for UsbStream {}
unsafe impl Sync for UsbStream {}

struct CtxPtr(*mut ffi::libusb_context);
unsafe impl Send for CtxPtr {}

impl UsbStream {
    /// Find and open the PureThermal device (desktop platforms).
    pub fn open() -> Result<Self, CameraError> {
        let mut ctx = ptr::null_mut();
        let r = unsafe { ffi::libusb_init(&mut ctx) };
        if r != 0 {
            return Err(CameraError::OpenFailed(format!("libusb_init: {}", err_name(r))));
        }
        match Self::find_and_open(ctx) {
            Ok(handle) => Self::from_handle(ctx, handle),
            Err(e) => {
                unsafe { ffi::libusb_exit(ctx) };
                Err(e)
            }
        }
    }

    /// Open from an already-opened OS file descriptor (Android `UsbDeviceConnection.getFileDescriptor()`).
    #[allow(dead_code)]
    pub fn open_fd(fd: isize) -> Result<Self, CameraError> {
        let mut ctx = ptr::null_mut();
        unsafe {
            // Device discovery is not permitted for apps on Android.
            ffi::libusb_set_option(ptr::null_mut(), LIBUSB_OPTION_NO_DEVICE_DISCOVERY);
            let r = ffi::libusb_init(&mut ctx);
            if r != 0 {
                return Err(CameraError::OpenFailed(format!("libusb_init: {}", err_name(r))));
            }
            let mut handle = ptr::null_mut();
            let r = ffi::libusb_wrap_sys_device(ctx, fd as _, &mut handle);
            if r != 0 {
                ffi::libusb_exit(ctx);
                return Err(CameraError::OpenFailed(format!("libusb_wrap_sys_device: {}", err_name(r))));
            }
            Self::from_handle(ctx, handle)
        }
    }

    /// Whether a PureThermal is attached (device list only; nothing is opened).
    #[cfg_attr(windows, allow(dead_code))] // Windows uses the driver check instead
    pub fn is_present() -> bool {
        unsafe {
            let mut ctx = ptr::null_mut();
            if ffi::libusb_init(&mut ctx) != 0 {
                return false;
            }
            let mut list = ptr::null();
            let n = ffi::libusb_get_device_list(ctx, &mut list);
            let mut found = false;
            for i in 0..n.max(0) as usize {
                let mut desc = std::mem::zeroed::<ffi::libusb_device_descriptor>();
                if ffi::libusb_get_device_descriptor(*list.add(i), &mut desc) == 0
                    && desc.idVendor == PT_VID
                    && desc.idProduct == PT_PID
                {
                    found = true;
                    break;
                }
            }
            if n >= 0 {
                ffi::libusb_free_device_list(list, 1);
            }
            ffi::libusb_exit(ctx);
            found
        }
    }

    fn find_and_open(ctx: *mut ffi::libusb_context) -> Result<*mut ffi::libusb_device_handle, CameraError> {
        unsafe {
            let mut list = ptr::null();
            let n = ffi::libusb_get_device_list(ctx, &mut list);
            if n < 0 {
                return Err(CameraError::OpenFailed(format!("get_device_list: {}", err_name(n as c_int))));
            }
            let mut result = Err(CameraError::DeviceNotFound);
            for i in 0..n as usize {
                let dev = *list.add(i);
                let mut desc = std::mem::zeroed::<ffi::libusb_device_descriptor>();
                if ffi::libusb_get_device_descriptor(dev, &mut desc) != 0 {
                    continue;
                }
                if desc.idVendor != PT_VID || desc.idProduct != PT_PID {
                    continue;
                }
                let mut handle = ptr::null_mut();
                let r = ffi::libusb_open(dev, &mut handle);
                result = if r == 0 {
                    Ok(handle)
                } else if r == LIBUSB_ERROR_ACCESS {
                    Err(CameraError::OpenFailed(
                        "Access denied. Linux: install the udev rule (see README-cross-platform.md). \
                         Windows: bind the PureThermal to WinUSB/libusbK with Zadig."
                            .into(),
                    ))
                } else if r == LIBUSB_ERROR_NOT_SUPPORTED {
                    Err(CameraError::OpenFailed(
                        "Device driver not supported by libusb. On Windows install WinUSB/libusbK \
                         for the PureThermal with Zadig."
                            .into(),
                    ))
                } else {
                    Err(CameraError::OpenFailed(format!("libusb_open: {}", err_name(r))))
                };
                break;
            }
            ffi::libusb_free_device_list(list, 1);
            result
        }
    }

    fn from_handle(
        ctx: *mut ffi::libusb_context,
        handle: *mut ffi::libusb_device_handle,
    ) -> Result<Self, CameraError> {
        let fail = |e: CameraError| {
            unsafe {
                ffi::libusb_close(handle);
                ffi::libusb_exit(ctx);
            }
            Err(e)
        };

        // Linux: let libusb detach/reattach uvcvideo around our claims.
        unsafe { ffi::libusb_set_auto_detach_kernel_driver(handle, 1) };

        let desc = match Self::read_config_descriptor(handle) {
            Ok(d) => d,
            Err(e) => return fail(e),
        };
        let config = match uvc_descriptors::parse_uvc_config(&desc) {
            Ok(c) => c,
            Err(e) => return fail(CameraError::UvcError(e)),
        };
        let vc_iface = find_interface(&desc, 0x0E, 0x01).unwrap_or(0);
        eprintln!(
            "[lepton-getthermal] UVC config: format={}, frame={}, {}x{}, interval={}, alt={}, ep=0x{:02X}, vc_if={}, vs_if={}",
            config.format_index, config.frame_index, config.width, config.height,
            config.frame_interval, config.alt_setting, config.endpoint_addr,
            vc_iface, config.vs_interface_num
        );

        let mut claimed = Vec::new();
        for iface in [vc_iface, config.vs_interface_num] {
            let r = unsafe { ffi::libusb_claim_interface(handle, iface as c_int) };
            if r != 0 {
                for &c in &claimed {
                    unsafe { ffi::libusb_release_interface(handle, c as c_int) };
                }
                return fail(CameraError::OpenFailed(format!(
                    "claim interface {iface}: {}",
                    err_name(r)
                )));
            }
            claimed.push(iface);
        }
        eprintln!("[lepton-getthermal] USB device opened (libusb)");

        Ok(Self { ctx, handle, config, vc_iface, claimed, active: Mutex::new(None) })
    }

    /// Read the full configuration descriptor via GET_DESCRIPTOR.
    fn read_config_descriptor(handle: *mut ffi::libusb_device_handle) -> Result<Vec<u8>, CameraError> {
        let get = |buf: &mut [u8]| unsafe {
            ffi::libusb_control_transfer(
                handle, 0x80, 0x06, 0x0200, 0,
                buf.as_mut_ptr(), buf.len() as u16, CTRL_TIMEOUT_MS,
            )
        };
        let mut head = [0u8; 9];
        let r = get(&mut head);
        if r < 9 {
            return Err(CameraError::UvcError(format!("GET_DESCRIPTOR(config) failed: {}", err_name(r))));
        }
        let total = u16::from_le_bytes([head[2], head[3]]) as usize;
        let mut buf = vec![0u8; total];
        let r = get(&mut buf);
        if r < 0 {
            return Err(CameraError::UvcError(format!("GET_DESCRIPTOR(config) failed: {}", err_name(r))));
        }
        buf.truncate(r as usize);
        Ok(buf)
    }

    fn control(&self, req_type: u8, req: u8, value: u16, index: u16, data: *mut u8, len: u16) -> c_int {
        unsafe { ffi::libusb_control_transfer(self.handle, req_type, req, value, index, data, len, CTRL_TIMEOUT_MS) }
    }

    /// Start Y16 video streaming with a frame callback.
    /// Callback receives: (y16_data: &[u8], width: u32, height: u32)
    pub fn start_stream<F>(&self, callback: F) -> Result<(), CameraError>
    where
        F: Fn(&[u8], u32, u32) + Send + Sync + 'static,
    {
        let mut active = self.active.lock();
        if active.is_some() {
            return Ok(());
        }

        // --- UVC probe / commit ---
        let vs = self.config.vs_interface_num as u16;
        let mut probe = UvcProbeControl {
            bm_hint: 0x0001,
            b_format_index: self.config.format_index,
            b_frame_index: self.config.frame_index,
            dw_frame_interval: self.config.frame_interval,
            dw_max_video_frame_size: (self.config.width as u32) * (self.config.height as u32) * 2,
            ..Default::default()
        };
        let p = &mut probe as *mut UvcProbeControl as *mut u8;
        let r = self.control(REQ_CLASS_IFACE_OUT, UVC_SET_CUR, VS_PROBE_CONTROL << 8, vs, p, 26);
        if r < 0 {
            return Err(CameraError::UvcError(format!("VS_PROBE SET_CUR failed: {}", err_name(r))));
        }
        let r = self.control(REQ_CLASS_IFACE_IN, UVC_GET_CUR, VS_PROBE_CONTROL << 8, vs, p, 26);
        if r < 0 {
            return Err(CameraError::UvcError(format!("VS_PROBE GET_CUR failed: {}", err_name(r))));
        }
        let (nf, nfr, ni) = (probe.b_format_index, probe.b_frame_index, { probe.dw_frame_interval });
        eprintln!("[lepton-getthermal] Probe negotiated: format={nf}, frame={nfr}, interval={ni}");
        let r = self.control(REQ_CLASS_IFACE_OUT, UVC_SET_CUR, VS_COMMIT_CONTROL << 8, vs, p, 26);
        if r < 0 {
            return Err(CameraError::UvcError(format!("VS_COMMIT SET_CUR failed: {}", err_name(r))));
        }

        // --- Activate isochronous alt setting ---
        let r = unsafe {
            ffi::libusb_set_interface_alt_setting(self.handle, vs as c_int, self.config.alt_setting as c_int)
        };
        if r != 0 {
            return Err(CameraError::StreamFailed(format!(
                "set alt setting {}: {}",
                self.config.alt_setting,
                err_name(r)
            )));
        }

        let packet_len = self.config.effective_max_packet() as usize;
        eprintln!("[lepton-getthermal] Iso packet size: {packet_len}");

        let state = Arc::new(StreamState {
            assembler: Mutex::new(FrameAssembler::new(self.config.width, self.config.height)),
            callback: Box::new(callback),
            width: self.config.width as u32,
            height: self.config.height as u32,
            packet_len,
            running: AtomicBool::new(true),
            in_flight: AtomicUsize::new(0),
        });

        // --- Allocate and submit transfers ---
        let mut transfers = Vec::with_capacity(NUM_TRANSFERS);
        for _ in 0..NUM_TRANSFERS {
            let xfer = unsafe { ffi::libusb_alloc_transfer(ISO_PACKETS_PER_TRANSFER as c_int) };
            if xfer.is_null() {
                break;
            }
            let mut buf = vec![0u8; packet_len * ISO_PACKETS_PER_TRANSFER];
            let buf_ptr = buf.as_mut_ptr();
            let ctx = Box::into_raw(Box::new(TransferCtx { state: state.clone(), _buf: buf }));
            unsafe {
                ffi::libusb_fill_iso_transfer(
                    xfer,
                    self.handle,
                    self.config.endpoint_addr,
                    buf_ptr,
                    (packet_len * ISO_PACKETS_PER_TRANSFER) as c_int,
                    ISO_PACKETS_PER_TRANSFER as c_int,
                    iso_callback,
                    ctx as *mut c_void,
                    0,
                );
                ffi::libusb_set_iso_packet_lengths(xfer, packet_len as u32);
            }
            state.in_flight.fetch_add(1, Ordering::SeqCst);
            let r = unsafe { ffi::libusb_submit_transfer(xfer) };
            if r != 0 {
                state.in_flight.fetch_sub(1, Ordering::SeqCst);
                eprintln!("[lepton-getthermal] submit iso transfer: {}", err_name(r));
            }
            transfers.push(xfer);
        }

        if state.in_flight.load(Ordering::SeqCst) == 0 {
            state.running.store(false, Ordering::SeqCst);
            unsafe {
                free_transfers(&transfers);
                ffi::libusb_set_interface_alt_setting(self.handle, vs as c_int, 0);
            }
            return Err(CameraError::StreamFailed("could not submit any isochronous transfer".into()));
        }

        // --- Event thread: drives callbacks until every transfer has retired ---
        let ev_state = state.clone();
        let ctx = CtxPtr(self.ctx);
        let event_thread = thread::Builder::new()
            .name("libusb-events".into())
            .spawn(move || {
                let ctx = ctx;
                let tv = libc::timeval { tv_sec: 0, tv_usec: 100_000 };
                while ev_state.in_flight.load(Ordering::SeqCst) > 0 {
                    unsafe { ffi::libusb_handle_events_timeout_completed(ctx.0, &tv, ptr::null_mut()) };
                }
                eprintln!("[lepton-getthermal] Streaming thread exited");
            })
            .map_err(|e| CameraError::StreamFailed(format!("spawn event thread: {e}")))?;

        *active = Some(ActiveStream { state, transfers: Transfers(transfers), event_thread });
        eprintln!("[lepton-getthermal] Streaming started");
        Ok(())
    }

    /// Stop streaming and release isochronous resources.
    pub fn stop_stream(&self) -> Result<(), CameraError> {
        let Some(active) = self.active.lock().take() else {
            return Ok(());
        };
        active.state.running.store(false, Ordering::SeqCst);
        for &x in &active.transfers.0 {
            unsafe { ffi::libusb_cancel_transfer(x) };
        }
        // The event thread exits once all transfers have called back.
        let deadline = Instant::now() + Duration::from_secs(3);
        while !active.event_thread.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        if active.event_thread.is_finished() {
            let _ = active.event_thread.join();
            unsafe { free_transfers(&active.transfers.0) };
        } else {
            // Should not happen; leak rather than free memory libusb may still use.
            eprintln!("[lepton-getthermal] WARNING: transfers did not retire in time");
        }
        unsafe {
            ffi::libusb_set_interface_alt_setting(self.handle, self.config.vs_interface_num as c_int, 0)
        };
        eprintln!("[lepton-getthermal] Streaming stopped");
        Ok(())
    }

    /// Send a UVC extension unit GET_CUR control transfer.
    pub fn get_ctrl(&self, unit_id: u8, control_id: u8, data: &mut [u8]) -> Result<usize, CameraError> {
        let index = ((unit_id as u16) << 8) | self.vc_iface as u16;
        let r = self.control(
            REQ_CLASS_IFACE_IN, UVC_GET_CUR, (control_id as u16) << 8, index,
            data.as_mut_ptr(), data.len() as u16,
        );
        if r < 0 {
            return Err(CameraError::LeptonError(format!(
                "GET_CUR failed (unit={unit_id}, ctrl={control_id}): {}",
                err_name(r)
            )));
        }
        Ok(r as usize)
    }

    /// Send a UVC extension unit SET_CUR control transfer.
    pub fn set_ctrl(&self, unit_id: u8, control_id: u8, data: &[u8]) -> Result<(), CameraError> {
        let index = ((unit_id as u16) << 8) | self.vc_iface as u16;
        let r = self.control(
            REQ_CLASS_IFACE_OUT, UVC_SET_CUR, (control_id as u16) << 8, index,
            data.as_ptr() as *mut u8, data.len() as u16,
        );
        if r < 0 {
            return Err(CameraError::LeptonError(format!(
                "SET_CUR failed (unit={unit_id}, ctrl={control_id}): {}",
                err_name(r)
            )));
        }
        Ok(())
    }
}

impl Drop for UsbStream {
    fn drop(&mut self) {
        let _ = self.stop_stream();
        unsafe {
            // Release VideoStreaming before VideoControl: libusb re-attaches the
            // kernel driver (uvcvideo) on release, and uvcvideo binds via the VC
            // interface, needing the VS interface to be free at that moment.
            for &i in self.claimed.iter().rev() {
                ffi::libusb_release_interface(self.handle, i as c_int);
            }
            ffi::libusb_close(self.handle);
            ffi::libusb_exit(self.ctx);
        }
        eprintln!("[lepton-getthermal] USB device closed");
    }
}

unsafe fn free_transfers(transfers: &[*mut ffi::libusb_transfer]) {
    for &x in transfers {
        let ctx = (*x).user_data as *mut TransferCtx;
        if !ctx.is_null() {
            drop(Box::from_raw(ctx));
        }
        ffi::libusb_free_transfer(x);
    }
}

/// Find the first interface number with the given class/subclass in a raw config descriptor.
fn find_interface(desc: &[u8], class: u8, subclass: u8) -> Option<u8> {
    let mut i = 0;
    while i + 1 < desc.len() {
        let len = desc[i] as usize;
        if len < 2 || i + len > desc.len() {
            break;
        }
        if desc[i + 1] == 0x04 && len >= 9 && desc[i + 5] == class && desc[i + 6] == subclass {
            return Some(desc[i + 2]);
        }
        i += len;
    }
    None
}

// ---------------------------------------------------------------------------
// Isochronous completion callback (runs on the libusb event thread)
// ---------------------------------------------------------------------------

extern "system" fn iso_callback(xfer: *mut ffi::libusb_transfer) {
    unsafe {
        let t = &mut *xfer;
        let ctx = &*(t.user_data as *const TransferCtx);
        let state = &ctx.state;

        let resubmit = state.running.load(Ordering::SeqCst)
            && t.status != LIBUSB_TRANSFER_CANCELLED
            && t.status != LIBUSB_TRANSFER_NO_DEVICE;

        if t.status == LIBUSB_TRANSFER_COMPLETED && state.running.load(Ordering::SeqCst) {
            let descs = std::slice::from_raw_parts(t.iso_packet_desc.as_ptr(), t.num_iso_packets as usize);
            for (i, d) in descs.iter().enumerate() {
                if d.status != LIBUSB_TRANSFER_COMPLETED || d.actual_length == 0 {
                    continue;
                }
                let packet = std::slice::from_raw_parts(t.buffer.add(i * state.packet_len), d.actual_length as usize);
                let mut asm = state.assembler.lock();
                match asm.feed(packet) {
                    Ok(Some(y16)) => {
                        drop(asm);
                        (state.callback)(&y16, state.width, state.height);
                    }
                    Ok(None) => {}
                    Err(e) => eprintln!("[lepton-getthermal] Frame assembly error: {e}"),
                }
            }
        } else if t.status != LIBUSB_TRANSFER_COMPLETED && t.status != LIBUSB_TRANSFER_CANCELLED {
            eprintln!("[lepton-getthermal] Iso transfer status: {}", t.status);
        }

        if resubmit && ffi::libusb_submit_transfer(xfer) == 0 {
            return;
        }
        state.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}
