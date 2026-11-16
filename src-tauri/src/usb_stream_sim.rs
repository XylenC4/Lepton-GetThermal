//! Simulated PureThermal / Lepton 3.5 (`cargo build --features sim`).
//!
//! Same API as the real backends. Streams a 160x120 radiometric (TLinear,
//! 0.01 K) scene at ~9 Hz: a room at 22 °C, a cold window, a hot mug with a
//! steam plume and a warm "person" walking across. Lepton SDK attributes are
//! kept in memory so reads return what was last written.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::camera::types::CameraError;

const W: usize = 160;
const H: usize = 120;

pub struct UsbStream {
    ctrls: Mutex<HashMap<(u8, u8), Vec<u8>>>,
    running: Arc<AtomicBool>,
    last: Arc<Mutex<Vec<u8>>>,
    thread: Mutex<Option<thread::JoinHandle<()>>>,
}

fn words(w: &[u16]) -> Vec<u8> {
    w.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// UVC control id from a Lepton command id (same mapping as lepton.rs).
fn cid(cmd: u16) -> u8 {
    (((cmd & 0xFF) >> 2) + 1) as u8
}

impl UsbStream {
    pub fn open() -> Result<Self, CameraError> {
        eprintln!("[sim] Simulated PureThermal opened");
        let mut c = HashMap::new();
        let mut part = b"500-0771-01 (SIM)".to_vec();
        part.resize(32, 0);
        c.insert((4, cid(0x081C)), part); // OEM part number
        c.insert((6, cid(0x0208)), words(&[0x1234, 0x5678, 0x0000, 0x0000])); // serial
        c.insert((6, cid(0x023C)), words(&[0; 16])); // FFC shutter mode (manual)
        c.insert((6, cid(0x0248)), words(&[0, 0])); // gain mode high
        c.insert((3, cid(0x0100)), words(&[0, 0])); // AGC off
        c.insert((7, cid(0x0300)), words(&[0, 0])); // polarity white-hot
        c.insert((5, cid(0x0EC4)), words(&[1, 0])); // TLinear 0.01 K
        c.insert((5, cid(0x0ECC)), words(&[59, 79, 60, 80])); // spotmeter ROI
        // RAD flux params: emissivity 1.0, TBkg/TWin/TAtm/TRefl 22 °C, tau 1.0, refl 0
        let k22 = (295.15f64 * 100.0) as u16;
        c.insert((5, cid(0x0EBC)), words(&[8192, k22, 8192, k22, 8192, k22, 0, k22]));
        Ok(Self {
            ctrls: Mutex::new(c),
            running: Arc::new(AtomicBool::new(false)),
            last: Arc::new(Mutex::new(Vec::new())),
            thread: Mutex::new(None),
        })
    }

    #[allow(dead_code)]
    pub fn open_fd(_fd: isize) -> Result<Self, CameraError> {
        Self::open()
    }

    #[allow(dead_code)]
    pub fn is_present() -> bool {
        true
    }

    pub fn start_stream<F>(&self, callback: F) -> Result<(), CameraError>
    where
        F: Fn(&[u8], u32, u32) + Send + Sync + 'static,
    {
        let mut t = self.thread.lock();
        if t.is_some() {
            return Ok(());
        }
        self.running.store(true, Ordering::SeqCst);
        let running = self.running.clone();
        let last = self.last.clone();
        *t = Some(thread::spawn(move || {
            let start = Instant::now();
            let mut seed: u32 = 0x9E37_79B9;
            let mut buf = vec![0u8; W * H * 2];
            while running.load(Ordering::SeqCst) {
                let t = start.elapsed().as_secs_f32();
                render(&mut buf, t, &mut seed);
                last.lock().clone_from(&buf);
                callback(&buf, W as u32, H as u32);
                thread::sleep(Duration::from_millis(111));
            }
        }));
        Ok(())
    }

    pub fn stop_stream(&self) -> Result<(), CameraError> {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.thread.lock().take() {
            let _ = h.join();
        }
        Ok(())
    }

    pub fn get_ctrl(&self, unit_id: u8, control_id: u8, data: &mut [u8]) -> Result<usize, CameraError> {
        // RAD spotmeter object (value, max, min, population) over the ROI.
        if unit_id == 5 && control_id == cid(0x0ED0) {
            let roi = self.ctrls.lock().get(&(5, cid(0x0ECC))).cloned().unwrap_or_default();
            let r: Vec<usize> = roi.chunks(2).map(|c| u16::from_le_bytes([c[0], c[1]]) as usize).collect();
            let (r0, c0, r1, c1) = (r[0], r[1], r[2].min(H - 1), r[3].min(W - 1));
            let f = self.last.lock();
            let (mut sum, mut n, mut mx, mut mn) = (0u64, 0u64, 0u16, u16::MAX);
            if !f.is_empty() {
                for y in r0..=r1 {
                    for x in c0..=c1 {
                        let i = (y * W + x) * 2;
                        let v = u16::from_le_bytes([f[i], f[i + 1]]);
                        sum += v as u64; n += 1; mx = mx.max(v); mn = mn.min(v);
                    }
                }
            }
            let avg = if n > 0 { (sum / n) as u16 } else { 29515 };
            let out = words(&[avg, mx, mn.min(avg), n as u16]);
            for (i, b) in data.iter_mut().enumerate() {
                *b = out.get(i).copied().unwrap_or(0);
            }
            return Ok(data.len());
        }
        let c = self.ctrls.lock();
        let v = c.get(&(unit_id, control_id));
        for (i, b) in data.iter_mut().enumerate() {
            *b = v.and_then(|v| v.get(i)).copied().unwrap_or(0);
        }
        Ok(data.len())
    }

    pub fn set_ctrl(&self, unit_id: u8, control_id: u8, data: &[u8]) -> Result<(), CameraError> {
        if data.len() > 1 {
            self.ctrls.lock().insert((unit_id, control_id), data.to_vec());
        }
        Ok(())
    }
}

impl Drop for UsbStream {
    fn drop(&mut self) {
        let _ = self.stop_stream();
    }
}

fn blob(x: f32, y: f32, cx: f32, cy: f32, rx: f32, ry: f32) -> f32 {
    let d = ((x - cx) / rx).powi(2) + ((y - cy) / ry).powi(2);
    (-d * d).exp() // flat-topped falloff
}

/// Writes one TLinear frame (kelvin * 100, little-endian).
fn render(buf: &mut [u8], t: f32, seed: &mut u32) {
    let px = 20.0 + ((t * 0.25).sin() * 0.5 + 0.5) * 100.0; // person walks left/right
    for y in 0..H {
        for x in 0..W {
            let (xf, yf) = (x as f32, y as f32);
            // room: slight vertical gradient (warmer ceiling)
            let mut c = 22.5 - yf / H as f32 * 2.0;
            // cold window (top left)
            if (8..48).contains(&x) && (8..50).contains(&y) {
                c = 9.0 + yf * 0.04;
            }
            // radiator under the window
            if (8..48).contains(&x) && (58..68).contains(&y) {
                c = 45.0 + ((xf * 0.8).sin() * 2.0);
            }
            // table surface
            if y >= 95 {
                c = 21.0;
            }
            // person: head + body
            let p = blob(xf, yf, px, 38.0, 9.0, 11.0).max(blob(xf, yf, px, 75.0, 16.0, 26.0));
            c = c + (34.0 - c) * p.min(1.0);
            // hot mug + steam
            let mug = blob(xf, yf, 132.0, 88.0, 9.0, 10.0);
            c = c + (62.0 - c) * mug;
            let sway = (yf * 0.15 + t * 2.0).sin() * 3.0;
            if y < 78 {
                let steam = blob(xf, yf, 132.0 + sway, 60.0, 5.0, 18.0) * 0.6;
                c = c + (40.0 - c) * steam;
            }
            // sensor noise ~0.05 K
            *seed ^= *seed << 13;
            *seed ^= *seed >> 17;
            *seed ^= *seed << 5;
            let n = (*seed as f32 / u32::MAX as f32 - 0.5) * 0.1;
            let raw = ((c + n + 273.15) * 100.0) as u16;
            let i = (y * W + x) * 2;
            buf[i..i + 2].copy_from_slice(&raw.to_le_bytes());
        }
    }
}
