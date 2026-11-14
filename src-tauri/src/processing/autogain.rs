//! Auto-gain processing for 16-bit thermal frames.
//! Normalizes Y16 (16-bit unsigned) pixel values to 8-bit (0-255)
//! using linear contrast stretching based on frame min/max.

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
}

/// Applies linear auto-gain to a Y16 frame buffer.
/// Input: raw 16-bit pixel values (little-endian u16 pairs).
/// Output: 8-bit normalized grayscale + frame statistics.
///
/// `range`: `None` stretches the frame's own min..max to 0..255 (auto).
/// `Some((lo, hi))` maps the fixed raw range lo..hi to 0..255 instead; values
/// outside it clip to 0 / 255. The reported min/max stats are always the
/// frame's real values.
pub fn auto_gain(
    y16_data: &[u8],
    width: usize,
    height: usize,
    range: Option<(u16, u16)>,
) -> GainResult {
    let pixel_count = width * height;
    if pixel_count == 0 || y16_data.len() < pixel_count * 2 {
        return GainResult {
            grayscale: Vec::new(),
            min_val: 0,
            max_val: 0,
            min_pos: 0,
            max_pos: 0,
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

    // Linear normalization to 0-255 over the auto (frame) or fixed (user) range.
    let (lo, hi) = match range {
        Some((a, b)) if b > a => (a, b),
        _ => (min_val, max_val),
    };
    let span = hi.saturating_sub(lo) as f64;
    let grayscale = pixels
        .iter()
        .map(|&val| {
            if span == 0.0 {
                0u8
            } else {
                let v = val.clamp(lo, hi);
                (((v - lo) as f64 / span) * 255.0) as u8
            }
        })
        .collect();

    GainResult {
        grayscale,
        min_val,
        max_val,
        min_pos,
        max_pos,
    }
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
}
