//! Difference mode: compare each frame against a recorded reference and show
//! only where the temperature changed.
//!
//! The reference stores, per pixel, the **minimum and maximum** raw Y16 value
//! seen over a few frames. That envelope is the pixel's own noise band: a pixel
//! only counts as changed when it rises above its max or drops below its min
//! (plus an extra threshold). Delta is measured from the envelope edge.
//!
//! Output: the live scene as a dimmed grayscale for context, with warmer pixels
//! tinted red -> yellow and colder pixels blue -> cyan, stronger with |delta|.

/// Per-pixel min/max envelope at sensor resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct Reference {
    pub min: Vec<u16>,
    pub max: Vec<u16>,
    pub width: usize,
    pub height: usize,
}

/// Parameters for one frame; values in raw Y16 units.
#[derive(Debug, Clone, Copy)]
pub struct DiffParams<'a> {
    pub reference: &'a Reference,
    /// Extra margin outside the min/max envelope that still counts as "no change".
    pub threshold: f32,
    /// Warming (above the envelope) that gets full color.
    pub warm_full: f32,
    /// Cooling (below the envelope) that gets full color.
    pub cold_full: f32,
}

const WARM_LO: [f32; 3] = [200.0, 30.0, 30.0];
const WARM_HI: [f32; 3] = [255.0, 235.0, 60.0];
const COLD_LO: [f32; 3] = [30.0, 70.0, 220.0];
const COLD_HI: [f32; 3] = [90.0, 235.0, 255.0];
/// Brightness of the unchanged context image.
const CONTEXT_GAIN: f32 = 0.45;

/// Signed change of one raw value against its envelope (0 inside min..max).
#[inline]
fn envelope_delta(raw: f32, min: f32, max: f32) -> f32 {
    if raw > max {
        raw - max
    } else if raw < min {
        raw - min
    } else {
        0.0
    }
}

/// Renders the difference view into `rgba` (display resolution).
/// `display_gs` is the normal grayscale (possibly upscaled) used as context;
/// `y16` / `width` / `height` are the raw sensor frame.
pub fn render(
    rgba: &mut [u8],
    display_gs: &[u8],
    display_w: usize,
    display_h: usize,
    y16: &[u8],
    width: usize,
    height: usize,
    p: DiffParams,
) {
    let pixel_count = display_w * display_h;
    let r = p.reference;
    if width == 0
        || height == 0
        || r.width != width
        || r.height != height
        || r.min.len() != width * height
        || r.max.len() != width * height
        || y16.len() < width * height * 2
        || rgba.len() < pixel_count * 4
        || display_gs.len() < pixel_count
    {
        return;
    }
    let threshold = p.threshold.max(0.0);
    let warm_span = (p.warm_full - threshold).max(1.0);
    let cold_span = (p.cold_full - threshold).max(1.0);

    for i in 0..pixel_count {
        let row = i / display_w;
        let col = i % display_w;
        let orig_row = ((row * height) / display_h).min(height - 1);
        let orig_col = ((col * width) / display_w).min(width - 1);
        let oi = orig_row * width + orig_col;
        let raw = u16::from_le_bytes([y16[oi * 2], y16[oi * 2 + 1]]) as f32;
        let delta = envelope_delta(raw, r.min[oi] as f32, r.max[oi] as f32);

        let gray = display_gs[i] as f32 * CONTEXT_GAIN;
        let mut out = [gray, gray, gray];

        let mag = delta.abs();
        if mag > threshold {
            let (lo, hi, span) = if delta > 0.0 {
                (WARM_LO, WARM_HI, warm_span)
            } else {
                (COLD_LO, COLD_HI, cold_span)
            };
            let t = ((mag - threshold) / span).clamp(0.0, 1.0);
            let alpha = 0.6 + 0.4 * t;
            for c in 0..3 {
                let tint = lo[c] + (hi[c] - lo[c]) * t;
                out[c] = gray * (1.0 - alpha) + tint * alpha;
            }
        }

        let base = i * 4;
        rgba[base] = out[0] as u8;
        rgba[base + 1] = out[1] as u8;
        rgba[base + 2] = out[2] as u8;
        rgba[base + 3] = 255;
    }
}

/// Records the per-pixel min/max envelope over N frames.
#[derive(Debug, Default)]
pub struct ReferenceRecorder {
    min: Vec<u16>,
    max: Vec<u16>,
    dims: (usize, usize),
    remaining: u32,
    total: u32,
}

impl ReferenceRecorder {
    pub fn start(&mut self, frames: u32) {
        self.total = frames.max(1);
        self.remaining = self.total;
        self.min.clear();
        self.max.clear();
        self.dims = (0, 0);
    }

    pub fn is_recording(&self) -> bool {
        self.remaining > 0
    }

    pub fn cancel(&mut self) {
        self.remaining = 0;
        self.min.clear();
        self.max.clear();
    }

    /// Adds a frame. Returns the finished reference after the last one.
    pub fn push(&mut self, y16: &[u8], width: usize, height: usize) -> Option<Reference> {
        if self.remaining == 0 {
            return None;
        }
        let n = width * height;
        if n == 0 || y16.len() < n * 2 {
            return None;
        }
        if self.dims != (width, height) || self.min.len() != n {
            // First frame (or the resolution changed): restart.
            self.min = vec![u16::MAX; n];
            self.max = vec![0; n];
            self.dims = (width, height);
            self.remaining = self.total;
        }
        for i in 0..n {
            let v = u16::from_le_bytes([y16[i * 2], y16[i * 2 + 1]]);
            if v < self.min[i] {
                self.min[i] = v;
            }
            if v > self.max[i] {
                self.max[i] = v;
            }
        }
        self.remaining -= 1;
        if self.remaining == 0 {
            Some(Reference {
                min: std::mem::take(&mut self.min),
                max: std::mem::take(&mut self.max),
                width,
                height,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y16(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn reference(min: &[u16], max: &[u16]) -> Reference {
        Reference { min: min.to_vec(), max: max.to_vec(), width: min.len(), height: 1 }
    }

    #[test]
    fn recorder_tracks_min_and_max() {
        let mut r = ReferenceRecorder::default();
        r.start(3);
        assert!(r.is_recording());
        assert!(r.push(&y16(&[100, 500]), 2, 1).is_none());
        assert!(r.push(&y16(&[120, 480]), 2, 1).is_none());
        let reference = r.push(&y16(&[90, 510]), 2, 1).expect("done after 3 frames");
        assert_eq!(reference.min, vec![90, 480]);
        assert_eq!(reference.max, vec![120, 510]);
        assert_eq!((reference.width, reference.height), (2, 1));
        assert!(!r.is_recording());
    }

    #[test]
    fn delta_is_zero_inside_the_envelope() {
        assert_eq!(envelope_delta(105.0, 100.0, 110.0), 0.0);
        assert_eq!(envelope_delta(115.0, 100.0, 110.0), 5.0);
        assert_eq!(envelope_delta(97.0, 100.0, 110.0), -3.0);
    }

    #[test]
    fn noise_inside_envelope_stays_gray_changes_are_tinted() {
        // Envelope 29980..30020 (±0.2 K noise). Pixels: noisy but inside,
        // +5 K warmer, -5 K colder (centikelvin).
        let r = reference(&[29980, 29980, 29980], &[30020, 30020, 30020]);
        let frame = y16(&[30015, 30520, 29480]);
        let gs = vec![200u8, 200, 200];
        let mut rgba = vec![0u8; 12];
        let p = DiffParams { reference: &r, threshold: 10.0, warm_full: 300.0, cold_full: 300.0 };
        render(&mut rgba, &gs, 3, 1, &frame, 3, 1, p);

        // Inside the envelope: neutral gray.
        assert_eq!(rgba[0], rgba[1]);
        assert_eq!(rgba[1], rgba[2]);
        // Warmer: red/yellow dominates over blue.
        assert!(rgba[4] > rgba[6] + 100);
        // Colder: blue dominates over red.
        assert!(rgba[10] > rgba[8] + 100);
    }

    #[test]
    fn warm_and_cold_scales_are_independent() {
        // Same +/-2 K change; warm full scale 2 K (saturated), cold 20 K (faint).
        let r = reference(&[30000, 30000], &[30000, 30000]);
        let frame = y16(&[30200, 29800]);
        let gs = vec![0u8, 0];
        let mut rgba = vec![0u8; 8];
        let p = DiffParams { reference: &r, threshold: 0.0, warm_full: 200.0, cold_full: 2000.0 };
        render(&mut rgba, &gs, 2, 1, &frame, 2, 1, p);
        // Warm pixel at full scale -> yellow (high green); cold pixel near the dark-blue end.
        assert!(rgba[1] > 200);
        assert!(rgba[5] < 120);
    }

    #[test]
    fn mismatched_reference_leaves_frame_untouched() {
        let r = reference(&[0, 0, 0, 0], &[0, 0, 0, 0]);
        let frame = y16(&[1, 2]);
        let mut rgba = vec![7u8; 8];
        let p = DiffParams { reference: &r, threshold: 1.0, warm_full: 10.0, cold_full: 10.0 };
        render(&mut rgba, &[0, 0], 2, 1, &frame, 2, 1, p);
        assert_eq!(rgba, vec![7u8; 8]);
    }
}
