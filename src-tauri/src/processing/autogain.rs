//! Auto-gain processing for 16-bit thermal frames.
//! Normalizes Y16 (16-bit unsigned) pixel values to 8-bit (0-255), either by
//! linear contrast stretching or by plateau histogram equalization (HEQ).

/// How raw values are mapped to the 8-bit grayscale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Contrast {
    /// Linear stretch of lo..hi to 0..255.
    #[default]
    Linear,
    /// Plateau histogram equalization: gray levels are spread over the
    /// temperatures that actually occur, so one hot object doesn't flatten
    /// the rest of the scene.
    Equalized,
}

/// Result of auto-gain processing, including metadata.
#[derive(Debug, Clone)]
pub struct GainResult {
    /// Normalized 8-bit grayscale buffer
    pub grayscale: Vec<u8>,
    /// Minimum raw value in frame
    pub min_val: u16,
    /// Maximum raw value in frame
    pub max_val: u16,
    /// Pixel index of minimum value
    pub min_pos: usize,
    /// Pixel index of maximum value
    pub max_pos: usize,
    /// Raw value at k/(SCALE_TICKS-1) of the gray scale, k = 0..SCALE_TICKS-1.
    /// Evenly spaced for `Linear`; follows the histogram for `Equalized`.
    pub scale_ticks: [u16; SCALE_TICKS],
}

/// Number of reference points reported for the color scale (every 6.25 %).
pub const SCALE_TICKS: usize = 17;

/// Max histogram bins for HEQ (TLinear 0.01 K: full resolution for scenes up to ~160 K wide).
const HEQ_MAX_BINS: usize = 16384;
/// Per-bin cap as a share of all pixels (~150 for 160x120, like FLIR's default plateau).
const HEQ_PLATEAU: f64 = 0.008;
/// Share of the linear mapping blended into HEQ; keeps flat scenes from turning into noise.
const HEQ_LINEAR_MIX: f64 = 0.2;

/// Applies linear auto-gain to a Y16 frame buffer (see [`gain`]).
pub fn auto_gain(
    y16_data: &[u8],
    width: usize,
    height: usize,
    range: Option<(u16, u16)>,
) -> GainResult {
    gain(y16_data, width, height, range, Contrast::Linear)
}

/// Normalizes a Y16 frame buffer to 8-bit.
/// Input: raw 16-bit pixel values (little-endian u16 pairs).
/// Output: 8-bit grayscale + frame statistics.
///
/// `range`: `None` uses the frame's own min..max (auto). `Some((lo, hi))`
/// uses the fixed raw range lo..hi instead; values outside it clip to 0 / 255.
/// The reported min/max stats are always the frame's real values.
pub fn gain(
    y16_data: &[u8],
    width: usize,
    height: usize,
    range: Option<(u16, u16)>,
    contrast: Contrast,
) -> GainResult {
    let pixel_count = width * height;
    if pixel_count == 0 || y16_data.len() < pixel_count * 2 {
        return GainResult {
            grayscale: Vec::new(),
            min_val: 0,
            max_val: 0,
            min_pos: 0,
            max_pos: 0,
            scale_ticks: [0; SCALE_TICKS],
        };
    }

    // Parse Y16 little-endian pixels and find min/max
    let mut min_val = u16::MAX;
    let mut max_val = u16::MIN;
    let mut min_pos = 0;
    let mut max_pos = 0;

    let pixels: Vec<u16> = (0..pixel_count)
        .map(|i| u16::from_le_bytes([y16_data[i * 2], y16_data[i * 2 + 1]]))
        .collect();

    for (i, &val) in pixels.iter().enumerate() {
        if val < min_val {
            min_val = val;
            min_pos = i;
        }
        if val > max_val {
            max_val = val;
            max_pos = i;
        }
    }

    let (lo, hi) = match range {
        Some((a, b)) if b > a => (a, b),
        _ => (min_val, max_val),
    };

    let (grayscale, scale_ticks) = if hi <= lo {
        (vec![0u8; pixel_count], [lo; SCALE_TICKS])
    } else {
        match contrast {
            Contrast::Linear => linear(&pixels, lo, hi),
            Contrast::Equalized => equalize(&pixels, lo, hi),
        }
    };

    GainResult {
        grayscale,
        min_val,
        max_val,
        min_pos,
        max_pos,
        scale_ticks,
    }
}

fn linear(pixels: &[u16], lo: u16, hi: u16) -> (Vec<u8>, [u16; SCALE_TICKS]) {
    let span = (hi - lo) as f64;
    let gs = pixels
        .iter()
        .map(|&val| (((val.clamp(lo, hi) - lo) as f64 / span) * 255.0) as u8)
        .collect();
    let mut ticks = [0u16; SCALE_TICKS];
    for (k, t) in ticks.iter_mut().enumerate() {
        *t = (lo as f64 + span * k as f64 / (SCALE_TICKS - 1) as f64).round() as u16;
    }
    (gs, ticks)
}

fn equalize(pixels: &[u16], lo: u16, hi: u16) -> (Vec<u8>, [u16; SCALE_TICKS]) {
    let span = (hi - lo) as usize + 1;
    let bin_w = span.div_ceil(HEQ_MAX_BINS).max(1);
    let nbins = span.div_ceil(bin_w);
    let bin_of = |v: u16| ((v.clamp(lo, hi) - lo) as usize / bin_w).min(nbins - 1);

    // Histogram with plateau clipping.
    let mut hist = vec![0u32; nbins];
    for &v in pixels {
        hist[bin_of(v)] += 1;
    }
    let plateau = ((pixels.len() as f64 * HEQ_PLATEAU).round() as u32).max(1);
    for h in hist.iter_mut() {
        *h = (*h).min(plateau);
    }

    // Gray level per bin: midpoint of the bin's share of the clipped CDF,
    // stretched so the lowest occupied bin maps to 0 and the highest to 1,
    // then blended with the linear position.
    let total: u64 = hist.iter().map(|&h| h as u64).sum();
    let mut level = vec![0f64; nbins];
    let mut cum = 0u64;
    for (b, &h) in hist.iter().enumerate() {
        let mid = cum as f64 + h as f64 / 2.0;
        cum += h as u64;
        level[b] = if total > 0 { mid / total as f64 } else { 0.0 };
    }
    let first = hist.iter().position(|&h| h > 0).unwrap_or(0);
    let last = hist.iter().rposition(|&h| h > 0).unwrap_or(nbins - 1);
    let (l0, l1) = (level[first], level[last]);
    let lin_den = (nbins - 1).max(1) as f64;
    for (b, l) in level.iter_mut().enumerate() {
        let heq = if l1 > l0 { ((*l - l0) / (l1 - l0)).clamp(0.0, 1.0) } else { 0.0 };
        let lin = b as f64 / lin_den;
        *l = (1.0 - HEQ_LINEAR_MIX) * heq + HEQ_LINEAR_MIX * lin;
    }

    let gs = pixels.iter().map(|&v| (level[bin_of(v)] * 255.0).round() as u8).collect();

    // Raw value where the gray level first reaches k/(SCALE_TICKS-1).
    let mut ticks = [lo; SCALE_TICKS];
    ticks[SCALE_TICKS - 1] = hi;
    for (k, t) in ticks.iter_mut().enumerate().take(SCALE_TICKS - 1).skip(1) {
        let target = k as f64 / (SCALE_TICKS - 1) as f64;
        let b = level.iter().position(|&l| l >= target).unwrap_or(nbins - 1);
        *t = (lo as usize + b * bin_w + bin_w / 2).min(hi as usize) as u16;
    }
    (gs, ticks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_y16_frame(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    #[test]
    fn auto_gain_normalizes_to_full_range() {
        // 4 pixels: min=100, max=200
        let frame = make_y16_frame(&[100, 150, 200, 100]);
        let result = auto_gain(&frame, 2, 2, None);

        assert_eq!(result.grayscale.len(), 4);
        assert_eq!(result.grayscale[0], 0);   // min → 0
        assert_eq!(result.grayscale[2], 255); // max → 255
        // midpoint: (150-100)/(200-100) * 255 ≈ 127
        assert!((result.grayscale[1] as i16 - 127).abs() <= 1);
        assert_eq!(result.scale_ticks[0], 100);
        assert_eq!(result.scale_ticks[4], 125);
        assert_eq!(result.scale_ticks[8], 150);
        assert_eq!(result.scale_ticks[16], 200);
    }

    #[test]
    fn auto_gain_reports_min_max() {
        let frame = make_y16_frame(&[300, 100, 500, 200]);
        let result = auto_gain(&frame, 2, 2, None);

        assert_eq!(result.min_val, 100);
        assert_eq!(result.max_val, 500);
        assert_eq!(result.min_pos, 1); // index of value 100
        assert_eq!(result.max_pos, 2); // index of value 500
    }

    #[test]
    fn auto_gain_uniform_frame() {
        // All same value → all output should be 0 (no division by zero)
        let frame = make_y16_frame(&[1000, 1000, 1000, 1000]);
        let result = auto_gain(&frame, 2, 2, None);

        assert_eq!(result.grayscale, vec![0, 0, 0, 0]);
        assert_eq!(result.min_val, 1000);
        assert_eq!(result.max_val, 1000);
        // Same for HEQ: no division by zero.
        let heq = gain(&frame, 2, 2, None, Contrast::Equalized);
        assert_eq!(heq.grayscale, vec![0, 0, 0, 0]);
    }

    #[test]
    fn fixed_range_clips_and_keeps_real_stats() {
        // Range 200..400: 100 clips to 0, 500 clips to 255, 300 is mid-scale.
        let frame = make_y16_frame(&[100, 300, 500, 200]);
        let result = auto_gain(&frame, 2, 2, Some((200, 400)));

        assert_eq!(result.grayscale[0], 0);
        assert!((result.grayscale[1] as i16 - 127).abs() <= 1);
        assert_eq!(result.grayscale[2], 255);
        assert_eq!(result.grayscale[3], 0);
        // Stats still describe the scene, not the display range.
        assert_eq!(result.min_val, 100);
        assert_eq!(result.max_val, 500);
    }

    #[test]
    fn invalid_fixed_range_falls_back_to_auto() {
        let frame = make_y16_frame(&[100, 150, 200, 100]);
        let auto = auto_gain(&frame, 2, 2, None);
        assert_eq!(auto_gain(&frame, 2, 2, Some((300, 300))).grayscale, auto.grayscale);
        assert_eq!(auto_gain(&frame, 2, 2, Some((400, 300))).grayscale, auto.grayscale);
    }

    #[test]
    fn auto_gain_empty_frame() {
        let result = auto_gain(&[], 0, 0, None);
        assert!(result.grayscale.is_empty());
    }

    /// A room at ~20 °C (with slight variation) plus one small hot object at 80 °C.
    /// Linear: the hot object takes most of the gray range, the room is squeezed
    /// into a few levels. HEQ: the room gets most of the levels.
    #[test]
    fn heq_spreads_the_scene_beside_a_hot_spot() {
        let mut vals: Vec<u16> = (0..19200u32).map(|i| 29315 + (i % 200) as u16).collect(); // 20.0..22.0 °C
        for v in vals.iter_mut().take(20) {
            *v = 35315; // 80 °C
        }
        let frame = make_y16_frame(&vals);
        let distinct = |gs: &[u8]| {
            let mut seen = [false; 256];
            for (i, &g) in gs.iter().enumerate() {
                if i >= 20 {
                    seen[g as usize] = true;
                }
            }
            seen.iter().filter(|&&s| s).count()
        };
        let lin = gain(&frame, 160, 120, None, Contrast::Linear);
        let heq = gain(&frame, 160, 120, None, Contrast::Equalized);
        assert!(distinct(&lin.grayscale) <= 10, "linear: {}", distinct(&lin.grayscale));
        assert!(distinct(&heq.grayscale) >= 150, "heq: {}", distinct(&heq.grayscale));
        // The hot spot is still the brightest, the coldest pixel the darkest.
        assert_eq!(heq.grayscale[0], 255);
        assert_eq!(heq.grayscale[200], 0); // 29315 = coldest
        // Ticks: ends are the range, the middle ticks sit inside the room band.
        assert_eq!(heq.scale_ticks[0], 29315);
        assert_eq!(heq.scale_ticks[16], 35315);
        assert!(heq.scale_ticks[8] >= 29315 && heq.scale_ticks[8] <= 29515, "{:?}", heq.scale_ticks);
        // Monotonic: warmer never maps darker.
        let mut pairs: Vec<(u16, u8)> = vals.iter().copied().zip(heq.grayscale.iter().copied()).collect();
        pairs.sort();
        assert!(pairs.windows(2).all(|w| w[0].1 <= w[1].1));
    }

    #[test]
    fn heq_respects_fixed_range() {
        let frame = make_y16_frame(&[100, 300, 500, 200]);
        let r = gain(&frame, 2, 2, Some((200, 400)), Contrast::Equalized);
        assert_eq!(r.grayscale[0], 0); // below range
        assert_eq!(r.grayscale[2], 255); // above range
        assert_eq!((r.scale_ticks[0], r.scale_ticks[16]), (200, 400));
    }
}
