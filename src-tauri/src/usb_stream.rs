//! Platform-specific USB backends with a common `UsbStream` API.
//! - macOS: native IOKit (isochronous via IOUSBInterfaceInterface)
//! - everything else (Linux, Windows, Android): libusb

#[cfg(target_os = "macos")]
#[path = "usb_stream_iokit.rs"]
mod imp;

#[cfg(not(target_os = "macos"))]
#[path = "usb_stream_libusb.rs"]
mod imp;

pub use imp::UsbStream;

/// Hardware-in-the-loop test. Needs a PureThermal attached:
/// `cargo test --lib hw_ -- --ignored --nocapture`
#[cfg(test)]
mod hw_tests {
    use super::UsbStream;
    use crate::camera::lepton::LeptonController;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    #[ignore]
    fn hw_stream_and_controls() {
        let usb = Arc::new(UsbStream::open().expect("open PureThermal"));
        let lep = LeptonController::new(usb.clone());
        eprintln!("part number : {:?}", lep.get_part_number());
        eprintln!("serial      : {:?}", lep.get_serial_number());
        eprintln!("radiometry  : {}", lep.supports_radiometry());
        eprintln!("ffc mode    : {:?}", lep.get_ffc_mode());
        eprintln!("agc enable  : {:?}", lep.get_agc_enable());
        eprintln!("polarity    : {:?}", lep.get_polarity());
        eprintln!("tlinear res : {:?}", lep.get_tlinear_resolution());
        eprintln!("gain mode   : {:?}", lep.get_gain_mode());
        let ffc = lep.get_ffc_mode().expect("ffc mode");
        lep.set_ffc_mode(ffc).expect("set ffc mode (round-trip)");
        lep.set_agc_enable(false).expect("agc off");
        assert!(lep.supports_radiometry());
        eprintln!("spot ROI    : {:?}", lep.get_spotmeter_roi());
        assert!(lep.get_part_number().is_ok(), "extension-unit control failed");

        for round in 0..2 {
            let frames = Arc::new(AtomicUsize::new(0));
            let last = Arc::new(parking_lot::Mutex::new(Vec::<u8>::new()));
            let (f, l) = (frames.clone(), last.clone());
            usb.start_stream(move |data, _w, _h| {
                f.fetch_add(1, Ordering::SeqCst);
                *l.lock() = data.to_vec();
            })
            .expect("start stream");
            std::thread::sleep(Duration::from_secs(3));
            usb.stop_stream().expect("stop");
            let n = frames.load(Ordering::SeqCst);
            let px: Vec<u16> = last.lock().chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
            let (mn, mx) = (px.iter().min().copied(), px.iter().max().copied());
            eprintln!("round {round}: {n} frames in 3 s, {} px, min={mn:?} max={mx:?}", px.len());
            if let (Some(mn), Some(mx)) = (mn, mx) {
                eprintln!("   ~ {:.1} .. {:.1} °C (TLinear 0.01K)", mn as f32 / 100.0 - 273.15, mx as f32 / 100.0 - 273.15);
                // dump a PGM for eyeballing
                let span = (mx - mn).max(1) as f32;
                let img: Vec<u8> = px.iter().map(|&v| ((v - mn) as f32 / span * 255.0) as u8).collect();
                let mut out = format!("P5 160 {} 255\n", px.len() / 160).into_bytes();
                out.extend(img);
                let _ = std::fs::write(std::env::temp_dir().join(format!("lepton-test-{round}.pgm")), out);
            }
            assert!(n >= 10, "expected ~27 frames, got {n}");
        }
    }

    /// Gain mode + RAD flux parameters (emissivity): read, write a changed
    /// emissivity, read back, restore. `cargo test --lib hw_rad -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn hw_rad_flux_and_gain() {
        use crate::camera::lepton::flux_emissivity_word;
        let usb = Arc::new(UsbStream::open().expect("open PureThermal"));
        let lep = LeptonController::new(usb);
        eprintln!("gain mode   : {:?} (0 high, 1 low, 2 auto)", lep.get_gain_mode());
        let orig = lep.get_flux_params().expect("read RAD flux params");
        eprintln!(
            "flux params : {orig:?}\n   emissivity {:.3}, TBkg {:.2} °C, tauWin {:.3}, TWin {:.2} °C, tauAtm {:.3}, TAtm {:.2} °C, reflWin {:.3}, TRefl {:.2} °C",
            orig[0] as f64 / 8192.0, orig[1] as f64 / 100.0 - 273.15,
            orig[2] as f64 / 8192.0, orig[3] as f64 / 100.0 - 273.15,
            orig[4] as f64 / 8192.0, orig[5] as f64 / 100.0 - 273.15,
            orig[6] as f64 / 8192.0, orig[7] as f64 / 100.0 - 273.15,
        );
        lep.set_emissivity(0.80, 22.0).expect("write emissivity");
        let changed = lep.get_flux_params().expect("read back");
        eprintln!("after write : {changed:?}");
        lep.set_flux_params(&orig).expect("restore");
        let restored = lep.get_flux_params().expect("read restored");
        eprintln!("restored    : {restored:?}");
        assert_eq!(changed[0], flux_emissivity_word(0.80));
        assert_eq!(changed[2..], orig[2..]); // window/atmosphere untouched
        assert_eq!(restored, orig);
    }
}
