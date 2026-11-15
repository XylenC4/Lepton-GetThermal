//! Display rotation in 90° steps (clockwise). Applied to the finished frame,
//! so everything the UI sees (image, raw values, min/max positions) agrees.

use super::FrameResult;

/// Clockwise quarter turns (0..=3) from degrees; anything else snaps to the nearest step.
pub fn quarter_turns(degrees: u32) -> u8 {
    (((degrees + 45) / 90) % 4) as u8
}

/// Index of sensor pixel `(x, y)` in the rotated `w`x`h` image.
pub fn rotate_index(idx: usize, w: usize, h: usize, q: u8) -> usize {
    let (x, y) = (idx % w, idx / w);
    let (u, v, rw) = match q % 4 {
        0 => (x, y, w),
        1 => (h - 1 - y, x, h),
        2 => (w - 1 - x, h - 1 - y, w),
        _ => (y, w - 1 - x, h),
    };
    v * rw + u
}

/// Rotates a `w`x`h` image of `px`-sized pixels (bytes). Returns the new buffer
/// and its width/height.
pub fn rotate_bytes(src: &[u8], w: usize, h: usize, px: usize, q: u8) -> (Vec<u8>, usize, usize) {
    let q = q % 4;
    let (rw, rh) = if q % 2 == 1 { (h, w) } else { (w, h) };
    if q == 0 || src.len() < w * h * px {
        return (src.to_vec(), w, h);
    }
    let mut out = vec![0u8; w * h * px];
    for i in 0..w * h {
        let j = rotate_index(i, w, h, q);
        out[j * px..(j + 1) * px].copy_from_slice(&src[i * px..(i + 1) * px]);
    }
    (out, rw, rh)
}

/// Rotates a `w`x`h` u16 image.
pub fn rotate_u16(src: &[u16], w: usize, h: usize, q: u8) -> Vec<u16> {
    if q % 4 == 0 || src.len() < w * h {
        return src.to_vec();
    }
    let mut out = vec![0u16; w * h];
    for (i, &v) in src.iter().enumerate().take(w * h) {
        out[rotate_index(i, w, h, q)] = v;
    }
    out
}

/// Rotates the image, the raw frame and the min/max positions of a frame result.
pub fn rotate_frame(r: &mut FrameResult, q: u8) {
    if q % 4 == 0 {
        return;
    }
    let (rgba, w, h) = rotate_bytes(&r.rgba, r.width, r.height, 4, q);
    r.rgba = rgba;
    r.width = w;
    r.height = h;

    let (sw, sh) = (r.sensor_width, r.sensor_height);
    if sw * sh > 0 {
        r.stats.min_pos = rotate_index(r.stats.min_pos.min(sw * sh - 1), sw, sh, q);
        r.stats.max_pos = rotate_index(r.stats.max_pos.min(sw * sh - 1), sw, sh, q);
    }
    let (raw, rw, rh) = rotate_bytes(&r.raw_y16, sw, sh, 2, q);
    r.raw_y16 = raw;
    r.sensor_width = rw;
    r.sensor_height = rh;
}

#[cfg(test)]
mod tests {
    use super::*;

    // 3x2 image:  0 1 2
    //             3 4 5
    const SRC: [u16; 6] = [0, 1, 2, 3, 4, 5];

    #[test]
    fn rotates_clockwise() {
        // 90° cw -> 2x3:  3 0 / 4 1 / 5 2
        assert_eq!(rotate_u16(&SRC, 3, 2, 1), vec![3, 0, 4, 1, 5, 2]);
        // 180°: 5 4 3 / 2 1 0
        assert_eq!(rotate_u16(&SRC, 3, 2, 2), vec![5, 4, 3, 2, 1, 0]);
        // 270° cw -> 2x3: 2 5 / 1 4 / 0 3
        assert_eq!(rotate_u16(&SRC, 3, 2, 3), vec![2, 5, 1, 4, 0, 3]);
        assert_eq!(rotate_u16(&SRC, 3, 2, 0), SRC.to_vec());
    }

    #[test]
    fn bytes_match_u16_and_four_turns_is_identity() {
        let bytes: Vec<u8> = SRC.iter().flat_map(|v| v.to_le_bytes()).collect();
        for q in 0..4u8 {
            let (b, w, h) = rotate_bytes(&bytes, 3, 2, 2, q);
            let back: Vec<u16> = b.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            assert_eq!(back, rotate_u16(&SRC, 3, 2, q));
            assert_eq!((w, h), if q % 2 == 1 { (2, 3) } else { (3, 2) });
        }
        let mut v = SRC.to_vec();
        let (mut w, mut h) = (3, 2);
        for _ in 0..4 {
            v = rotate_u16(&v, w, h, 1);
            (w, h) = (h, w);
        }
        assert_eq!(v, SRC.to_vec());
    }

    #[test]
    fn index_follows_pixel() {
        for q in 0..4u8 {
            let r = rotate_u16(&SRC, 3, 2, q);
            for i in 0..6 {
                assert_eq!(r[rotate_index(i, 3, 2, q)], SRC[i]);
            }
        }
        assert_eq!(quarter_turns(0), 0);
        assert_eq!(quarter_turns(90), 1);
        assert_eq!(quarter_turns(270), 3);
        assert_eq!(quarter_turns(360), 0);
    }
}
