use std::sync::{
    atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering},
    Arc,
};

use parking_lot::Mutex;

use crate::processing::{
    self,
    autogain::{Contrast, HeqState},
    diff::{Reference, ReferenceRecorder},
    hold::{Hold, HoldMode},
    palettes::Palette,
    upscale::Upscaler,
    FrameResult,
};
use crate::usb_stream::UsbStream;

use super::types::*;

pub struct CameraAcquisition {
    stream: Arc<UsbStream>,
    streaming: Arc<AtomicBool>,
    current_palette: Arc<Mutex<Palette>>,
    inverted: Arc<AtomicBool>,
    /// Raw Y16 threshold for isotherm overlay. 0 = disabled.
    isotherm_raw: Arc<AtomicU16>,
    /// Fixed display range packed as (lo << 16) | hi. 0 = auto (per-frame min/max).
    display_range: Arc<AtomicU32>,
    /// Difference mode (reference frame + settings).
    diff: Arc<Mutex<DiffState>>,
    /// Peak hold (max/min per pixel since start).
    hold: Arc<Mutex<Hold>>,
    /// Plateau histogram equalization instead of a linear stretch.
    equalize: Arc<AtomicBool>,
    /// Previous equalization mapping (temporal smoothing).
    heq_state: Arc<Mutex<HeqState>>,
    /// Neural network upscaler (None = disabled).
    upscaler: Arc<Mutex<Option<Upscaler>>>,
    /// Display rotation, clockwise quarter turns (0..=3).
    rotation: Arc<AtomicU8>,
    /// Whether upscaling is enabled by the user.
    upscale_enabled: Arc<AtomicBool>,
}

unsafe impl Send for CameraAcquisition {}

impl CameraAcquisition {
    pub fn new(stream: Arc<UsbStream>) -> Self {
        Self {
            stream,
            streaming: Arc::new(AtomicBool::new(false)),
            current_palette: Arc::new(Mutex::new(Palette::IronBlack)),
            inverted: Arc::new(AtomicBool::new(false)),
            isotherm_raw: Arc::new(AtomicU16::new(0)),
            display_range: Arc::new(AtomicU32::new(0)),
            diff: Arc::new(Mutex::new(DiffState::default())),
            hold: Arc::new(Mutex::new(Hold::default())),
            equalize: Arc::new(AtomicBool::new(false)),
            heq_state: Arc::new(Mutex::new(HeqState::default())),
            upscaler: Arc::new(Mutex::new(None)),
            upscale_enabled: Arc::new(AtomicBool::new(false)),
            rotation: Arc::new(AtomicU8::new(0)),
        }
    }

    pub fn set_palette(&self, palette: Palette) {
        *self.current_palette.lock() = palette;
    }

    pub fn set_inverted(&self, invert: bool) {
        self.inverted.store(invert, Ordering::Relaxed);
    }

    pub fn set_isotherm_raw(&self, threshold: u16) {
        self.isotherm_raw.store(threshold, Ordering::Relaxed);
    }

    /// Linear stretch (false) or plateau histogram equalization (true).
    pub fn set_equalize(&self, on: bool) {
        self.equalize.store(on, Ordering::Relaxed);
    }

    /// Peak hold off / max / min. Changing the mode restarts the hold.
    pub fn set_hold_mode(&self, mode: HoldMode) {
        self.hold.lock().set_mode(mode);
    }

    /// Restart the peak hold from the next frame.
    pub fn reset_hold(&self) {
        self.hold.lock().reset();
    }

    /// Fix the color mapping to raw Y16 `lo..hi`. `None` (or lo >= hi) = auto.
    pub fn set_display_range(&self, range: Option<(u16, u16)>) {
        let packed = match range {
            Some((lo, hi)) if hi > lo => ((lo as u32) << 16) | hi as u32,
            _ => 0,
        };
        self.display_range.store(packed, Ordering::Relaxed);
    }

    /// Difference mode: start recording a reference (per-pixel min/max over `frames`).
    pub fn record_reference(&self, frames: u32) {
        self.diff.lock().recorder.start(frames);
    }

    /// Difference mode: drop the reference (and any recording in progress).
    pub fn clear_reference(&self) {
        let mut d = self.diff.lock();
        d.recorder.cancel();
        d.reference = None;
    }

    /// Difference mode: a copy of the stored reference (per-pixel min/max), if any,
    /// rotated like the displayed frame.
    pub fn reference(&self) -> Option<Reference> {
        let r = self.diff.lock().reference.clone()?;
        let q = self.rotation.load(Ordering::Relaxed);
        if q == 0 {
            return Some(r);
        }
        use processing::rotate::rotate_u16;
        let (w, h) = if q % 2 == 1 { (r.height, r.width) } else { (r.width, r.height) };
        Some(Reference {
            min: rotate_u16(&r.min, r.width, r.height, q),
            max: rotate_u16(&r.max, r.width, r.height, q),
            width: w,
            height: h,
        })
    }

    /// Display rotation in degrees clockwise (0, 90, 180, 270).
    pub fn set_rotation(&self, degrees: u32) {
        self.rotation
            .store(processing::rotate::quarter_turns(degrees), Ordering::Relaxed);
    }

    /// Difference mode settings, in raw Y16 units.
    pub fn set_diff(&self, enabled: bool, threshold: f32, warm_full: f32, cold_full: f32) {
        let mut d = self.diff.lock();
        d.enabled = enabled;
        d.threshold = threshold.max(0.0);
        d.warm_full = warm_full.max(d.threshold + 1.0);
        d.cold_full = cold_full.max(d.threshold + 1.0);
    }

    /// Enable or disable neural network upscaling.
    /// Lazily initializes the ONNX model on first enable.
    pub fn set_upscale(&self, enabled: bool) {
        if enabled {
            let mut guard = self.upscaler.lock();
            if guard.is_none() {
                match Upscaler::new() {
                    Ok(up) => {
                        eprintln!("[lepton-getthermal] Upscaler initialized ({}x)", up.scale);
                        *guard = Some(up);
                    }
                    Err(e) => {
                        eprintln!("[lepton-getthermal] Failed to init upscaler: {e}");
                        return;
                    }
                }
            }
        }
        self.upscale_enabled.store(enabled, Ordering::Relaxed);
    }

    pub fn is_streaming(&self) -> bool {
        self.streaming.load(Ordering::Relaxed)
    }

    pub fn start_stream<F>(&self, on_frame: F) -> Result<(), CameraError>
    where
        F: Fn(FrameResult) + Send + Sync + 'static,
    {
        let palette = self.current_palette.clone();
        let inverted = self.inverted.clone();
        let isotherm_raw = self.isotherm_raw.clone();
        let display_range = self.display_range.clone();
        let upscaler = self.upscaler.clone();
        let upscale_enabled = self.upscale_enabled.clone();
        let diff = self.diff.clone();
        let hold = self.hold.clone();
        let equalize = self.equalize.clone();
        let heq_state = self.heq_state.clone();
        let rotation = self.rotation.clone();

        self.stream.start_stream(move |y16_data, width, height| {
            let current_palette = *palette.lock();
            let invert = inverted.load(Ordering::Relaxed);
            let iso = isotherm_raw.load(Ordering::Relaxed);
            let range = unpack_range(display_range.load(Ordering::Relaxed));
            let (w, h) = (width as usize, height as usize);

            // Difference mode: feed the recorder, then use the reference if it fits.
            let mut diff_guard = diff.lock();
            let ds = &mut *diff_guard;
            if let Some(reference) = ds.recorder.push(y16_data, w, h) {
                let noise = reference
                    .min
                    .iter()
                    .zip(&reference.max)
                    .map(|(lo, hi)| (hi - lo) as u64)
                    .sum::<u64>() as f64
                    / reference.min.len().max(1) as f64;
                eprintln!(
                    "[lepton-getthermal] Reference recorded ({}x{}), mean min/max spread {:.1} raw",
                    reference.width, reference.height, noise
                );
                ds.reference = Some(reference);
            }
            let recording = ds.recorder.is_recording();
            let reference_ready = ds
                .reference
                .as_ref()
                .is_some_and(|r| (r.width, r.height) == (w, h));
            let diff_params = match (&ds.reference, ds.enabled && reference_ready) {
                (Some(reference), true) => Some(processing::diff::DiffParams {
                    reference,
                    threshold: ds.threshold,
                    warm_full: ds.warm_full,
                    cold_full: ds.cold_full,
                }),
                _ => None,
            };

            // Peak hold: the held frame replaces the live one for everything that
            // follows (colors, stats, markers, pointer readout, difference view).
            let mut hold_guard = hold.lock();
            let held = hold_guard.apply(y16_data, w, h);
            let hold_frames = hold_guard.frames();
            drop(hold_guard);
            let frame: &[u8] = held.as_deref().unwrap_or(y16_data);

            let contrast = if equalize.load(Ordering::Relaxed) {
                Contrast::Equalized
            } else {
                Contrast::Linear
            };
            let mut heq_guard = heq_state.lock();
            if contrast == Contrast::Linear {
                heq_guard.reset(); // start fresh when equalization is switched on again
            }

            let mut up_guard = upscaler.lock();
            let up_ref = if upscale_enabled.load(Ordering::Relaxed) {
                up_guard.as_mut()
            } else {
                None
            };

            let mut result = processing::process_frame(
                frame,
                w,
                h,
                current_palette,
                invert,
                iso,
                range,
                contrast,
                Some(&mut *heq_guard),
                diff_params,
                up_ref,
            );
            drop(up_guard);
            drop(diff_guard);
            result.reference_ready = reference_ready;
            result.recording_reference = recording;
            result.hold_frames = hold_frames;
            processing::rotate::rotate_frame(&mut result, rotation.load(Ordering::Relaxed));
            on_frame(result);
        })?;

        self.streaming.store(true, Ordering::Relaxed);
        Ok(())
    }

    pub fn stop_stream(&self) {
        if self.streaming.load(Ordering::Relaxed) {
            let _ = self.stream.stop_stream();
            self.streaming.store(false, Ordering::Relaxed);
        }
    }
}

fn unpack_range(packed: u32) -> Option<(u16, u16)> {
    if packed == 0 {
        None
    } else {
        Some(((packed >> 16) as u16, packed as u16))
    }
}

/// Difference mode state. Units: raw Y16.
#[derive(Default)]
struct DiffState {
    enabled: bool,
    threshold: f32,
    warm_full: f32,
    cold_full: f32,
    recorder: ReferenceRecorder,
    reference: Option<Reference>,
}
