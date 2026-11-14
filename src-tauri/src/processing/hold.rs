//! Peak hold: keep each pixel's highest (max-hold) or lowest (min-hold) raw
//! value since the hold was started. Shows intermittent hot or cold spots,
//! e.g. a component that only heats up under load.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HoldMode {
    #[default]
    Off,
    Max,
    Min,
}

#[derive(Debug, Default)]
pub struct Hold {
    mode: HoldMode,
    buf: Vec<u16>,
    dims: (usize, usize),
    frames: u32,
}

impl Hold {
    pub fn mode(&self) -> HoldMode {
        self.mode
    }

    /// Frames accumulated since the hold started (0 when off).
    pub fn frames(&self) -> u32 {
        self.frames
    }

    /// Switch mode; any change (or `reset`) starts accumulating from scratch.
    pub fn set_mode(&mut self, mode: HoldMode) {
        if mode != self.mode {
            self.mode = mode;
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.buf.clear();
        self.frames = 0;
    }

    /// Adds a live frame and returns the held frame as Y16 LE bytes, or
    /// `None` when the hold is off (use the live frame then).
    pub fn apply(&mut self, y16: &[u8], width: usize, height: usize) -> Option<Vec<u8>> {
        if self.mode == HoldMode::Off {
            return None;
        }
        let n = width * height;
        if n == 0 || y16.len() < n * 2 {
            return None;
        }
        if self.dims != (width, height) || self.buf.len() != n {
            self.buf = vec![if self.mode == HoldMode::Max { 0 } else { u16::MAX }; n];
            self.dims = (width, height);
            self.frames = 0;
        }
        let max = self.mode == HoldMode::Max;
        for (i, held) in self.buf.iter_mut().enumerate() {
            let v = u16::from_le_bytes([y16[i * 2], y16[i * 2 + 1]]);
            if (max && v > *held) || (!max && v < *held) {
                *held = v;
            }
        }
        self.frames = self.frames.saturating_add(1);
        Some(self.buf.iter().flat_map(|v| v.to_le_bytes()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y16(values: &[u16]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn values(bytes: &[u8]) -> Vec<u16> {
        bytes.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect()
    }

    #[test]
    fn off_passes_through() {
        let mut h = Hold::default();
        assert!(h.apply(&y16(&[1, 2]), 2, 1).is_none());
        assert_eq!(h.frames(), 0);
    }

    #[test]
    fn max_hold_keeps_peaks() {
        let mut h = Hold::default();
        h.set_mode(HoldMode::Max);
        h.apply(&y16(&[100, 500]), 2, 1);
        let out = h.apply(&y16(&[300, 200]), 2, 1).unwrap();
        assert_eq!(values(&out), vec![300, 500]);
        assert_eq!(h.frames(), 2);
    }

    #[test]
    fn min_hold_keeps_lows_and_reset_restarts() {
        let mut h = Hold::default();
        h.set_mode(HoldMode::Min);
        h.apply(&y16(&[100, 500]), 2, 1);
        let out = h.apply(&y16(&[300, 200]), 2, 1).unwrap();
        assert_eq!(values(&out), vec![100, 200]);
        h.reset();
        let out = h.apply(&y16(&[300, 400]), 2, 1).unwrap();
        assert_eq!(values(&out), vec![300, 400]);
        assert_eq!(h.frames(), 1);
    }

    #[test]
    fn switching_mode_restarts() {
        let mut h = Hold::default();
        h.set_mode(HoldMode::Max);
        h.apply(&y16(&[900]), 1, 1);
        h.set_mode(HoldMode::Min);
        let out = h.apply(&y16(&[300]), 1, 1).unwrap();
        assert_eq!(values(&out), vec![300]);
    }
}
