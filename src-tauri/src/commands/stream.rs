use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

use crate::camera::acquisition::CameraAcquisition;
use crate::camera::lepton::LeptonController;
use crate::processing::palettes::Palette;
use crate::usb_stream::UsbStream;
use crate::AppState;

#[derive(Clone, Serialize)]
struct FrameEvent {
    data: String,
    width: usize,
    height: usize,
    min_val: u16,
    max_val: u16,
    min_pos: usize,
    max_pos: usize,
    reference_ready: bool,
    recording_reference: bool,
    /// Raw Y16 (LE, base64) of the displayed frame, sensor resolution: pointer readout.
    raw: String,
    raw_width: usize,
    raw_height: usize,
    /// Peak hold: frames accumulated (0 = off).
    hold_frames: u32,
    /// Raw values at every 1/16 of the color scale (non-linear with HEQ).
    scale_ticks: [u16; crate::processing::autogain::SCALE_TICKS],
}

#[tauri::command]
pub fn connect_camera(state: State<'_, AppState>) -> Result<String, String> {
    eprintln!("[lepton-getthermal] Connecting via USB...");

    // Windows: make sure the PureThermal is bound to WinUSB/libusbK before libusb
    // tries to open it, and explain how to fix it if not.
    let driver = crate::usb_driver_check::check();
    eprintln!("[lepton-getthermal] USB driver check: {:?} - {}", driver.state, driver.summary);
    for d in &driver.details {
        eprintln!("[lepton-getthermal]   {d}");
    }
    if !driver.usable() {
        return Err(driver.message());
    }

    let stream = Arc::new(UsbStream::open().map_err(|e| {
        eprintln!("[lepton-getthermal] USB open failed: {e}");
        if driver.hint.is_empty() {
            e.to_string()
        } else {
            format!("{e} ({})", driver.message())
        }
    })?);
    eprintln!("[lepton-getthermal] USB device opened");

    let cam = CameraAcquisition::new(stream.clone());
    *state.camera.lock() = Some(cam);

    let lepton = Arc::new(LeptonController::new(stream));
    // Force AGC off to preserve raw radiometric Y16 values
    let _ = lepton.set_agc_enable(false);
    // Disable auto-FFC to prevent random shutter clicks during streaming
    match lepton.get_ffc_mode() {
        Ok(mode) => eprintln!("[lepton-getthermal] FFC shutter mode: {} ({})",
            mode, if mode == 0 { "manual" } else { "auto" }),
        Err(e) => eprintln!("[lepton-getthermal] Failed to read FFC mode: {e}"),
    }
    let _ = lepton.set_ffc_mode(0); // 0 = manual
    let part = lepton.get_part_number().unwrap_or_default();
    eprintln!("[lepton-getthermal] Lepton controller ready, part: {part}");

    *state.lepton.lock() = Some(lepton);
    Ok(part)
}

/// Which USB driver Windows has bound to the PureThermal (WinUSB / libusbK /
/// usbvideo / ...), with a fix-it hint. Not applicable on Linux/macOS.
#[tauri::command]
pub fn get_usb_driver_status() -> crate::usb_driver_check::DriverReport {
    crate::usb_driver_check::check()
}

#[tauri::command]
pub fn start_stream(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    eprintln!("[lepton-getthermal] start_stream called");
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;

    cam.start_stream(move |frame_result| {
        let event = FrameEvent {
            data: BASE64.encode(&frame_result.rgba),
            width: frame_result.width,
            height: frame_result.height,
            min_val: frame_result.stats.min_val,
            max_val: frame_result.stats.max_val,
            min_pos: frame_result.stats.min_pos,
            max_pos: frame_result.stats.max_pos,
            reference_ready: frame_result.reference_ready,
            recording_reference: frame_result.recording_reference,
            raw: BASE64.encode(&frame_result.raw_y16),
            raw_width: frame_result.sensor_width,
            raw_height: frame_result.sensor_height,
            hold_frames: frame_result.hold_frames,
            scale_ticks: frame_result.stats.scale_ticks,
        };
        let _ = app.emit("thermal-frame", event);
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_stream(state: State<'_, AppState>) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    if let Some(cam) = cam_guard.as_ref() {
        cam.stop_stream();
    }
    Ok(())
}

#[tauri::command]
pub fn set_palette(state: State<'_, AppState>, palette: String) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    let p = match palette.as_str() {
        "ironblack" => Palette::IronBlack,
        "rainbow" => Palette::Rainbow,
        "grayscale" => Palette::Grayscale,
        _ => return Err(format!("Unknown palette: {palette}")),
    };
    cam.set_palette(p);
    Ok(())
}

#[tauri::command]
pub fn set_polarity(state: State<'_, AppState>, polarity: u32) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.set_inverted(polarity != 0);
    Ok(())
}

#[tauri::command]
pub fn set_isotherm(state: State<'_, AppState>, raw_threshold: u16) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.set_isotherm_raw(raw_threshold);
    Ok(())
}

/// Fix the color mapping to raw Y16 `min_raw..max_raw`. Pass null for either
/// value (or min >= max) to return to automatic per-frame min/max.
#[tauri::command]
pub fn set_display_range(
    state: State<'_, AppState>,
    min_raw: Option<u16>,
    max_raw: Option<u16>,
) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.set_display_range(min_raw.zip(max_raw));
    Ok(())
}

#[tauri::command]
pub fn set_upscale(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    eprintln!("[lepton-getthermal] set_upscale called: enabled={enabled}");
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.set_upscale(enabled);
    eprintln!("[lepton-getthermal] set_upscale done");
    Ok(())
}

/// Difference mode: record a reference = per-pixel min/max over `frames` (default 16).
#[tauri::command]
pub fn record_reference(state: State<'_, AppState>, frames: Option<u32>) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    let n = frames.unwrap_or(16).clamp(1, 128);
    eprintln!("[lepton-getthermal] Recording reference ({n} frames)");
    cam.record_reference(n);
    Ok(())
}

/// Difference mode: forget the reference frame.
#[tauri::command]
pub fn clear_reference(state: State<'_, AppState>) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.clear_reference();
    Ok(())
}

/// Difference mode on/off and its scale, in raw Y16 units
/// (TLinear 0.01 K: 100 = 1 degree).
#[tauri::command]
pub fn set_diff_mode(
    state: State<'_, AppState>,
    enabled: bool,
    threshold_raw: f32,
    warm_full_raw: f32,
    cold_full_raw: f32,
) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.set_diff(enabled, threshold_raw, warm_full_raw, cold_full_raw);
    Ok(())
}

/// Contrast: "linear" (stretch) or "equalized" (plateau histogram equalization).
#[tauri::command]
pub fn set_contrast_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    match mode.as_str() {
        "linear" => cam.set_equalize(false),
        "equalized" => cam.set_equalize(true),
        _ => return Err(format!("Unknown contrast mode: {mode}")),
    }
    Ok(())
}

/// Peak hold: "off", "max" (hottest value per pixel) or "min" (coldest).
#[tauri::command]
pub fn set_hold_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    use crate::processing::hold::HoldMode;
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    let m = match mode.as_str() {
        "off" => HoldMode::Off,
        "max" => HoldMode::Max,
        "min" => HoldMode::Min,
        _ => return Err(format!("Unknown hold mode: {mode}")),
    };
    cam.set_hold_mode(m);
    Ok(())
}

/// Peak hold: start accumulating again from the next frame.
#[tauri::command]
pub fn reset_hold(state: State<'_, AppState>) -> Result<(), String> {
    let cam_guard = state.camera.lock();
    let cam = cam_guard.as_ref().ok_or("Camera not connected")?;
    cam.reset_hold();
    Ok(())
}
