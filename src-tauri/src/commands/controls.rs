use tauri::State;

use crate::camera::types::DeviceInfo;
use crate::AppState;

fn with_lepton<T: std::fmt::Debug>(
    state: &State<'_, AppState>,
    cmd_name: &str,
    f: impl FnOnce(
        &crate::camera::lepton::LeptonController,
    ) -> Result<T, crate::camera::types::CameraError>,
) -> Result<T, String> {
    let guard = state.lepton.lock();
    let lepton = guard.as_ref().ok_or("Camera not connected")?;
    match f(lepton) {
        Ok(val) => {
            eprintln!("[lepton-getthermal] {cmd_name}: OK ({val:?})");
            Ok(val)
        }
        Err(e) => {
            eprintln!("[lepton-getthermal] {cmd_name}: ERROR: {e}");
            Err(e.to_string())
        }
    }
}

#[tauri::command]
pub fn perform_ffc(state: State<'_, AppState>) -> Result<(), String> {
    with_lepton(&state, "perform_ffc", |l| l.perform_ffc())
}

#[tauri::command]
pub fn get_polarity(state: State<'_, AppState>) -> Result<u16, String> {
    with_lepton(&state, "get_polarity", |l| l.get_polarity())
}

#[tauri::command]
pub fn get_device_info(state: State<'_, AppState>) -> Result<DeviceInfo, String> {
    with_lepton(&state, "get_device_info", |l| {
        let part = l.get_part_number()?;
        let serial = l.get_serial_number()?;
        let radiometry = l.supports_radiometry();

        Ok(DeviceInfo {
            serial_number: format!("{serial}"),
            part_number: part,
            firmware_version: String::new(), // TODO: read from OEM
            supports_radiometry: radiometry,
            supports_hw_pseudo_color: true,
            width: 0,
            height: 0,
            fps: 0,
            usb_driver: crate::usb_driver_check::check().driver,
        })
    })
}

#[tauri::command]
pub fn get_spotmeter_roi(state: State<'_, AppState>) -> Result<[u16; 4], String> {
    with_lepton(&state, "get_spotmeter_roi", |l| l.get_spotmeter_roi())
}

#[tauri::command]
pub fn set_spotmeter_roi(
    state: State<'_, AppState>,
    row_start: u16,
    col_start: u16,
    row_end: u16,
    col_end: u16,
) -> Result<(), String> {
    with_lepton(&state, "set_spotmeter_roi", |l| {
        l.set_spotmeter_roi([row_start, col_start, row_end, col_end])
    })
}

/// Get spot temperature in Celsius from the radiometry spotmeter.
#[tauri::command]
pub fn get_spot_temperature(state: State<'_, AppState>) -> Result<f64, String> {
    with_lepton(&state, "get_spot_temperature", |l| {
        let resolution = l.get_tlinear_resolution()?;
        // Read spotmeter object (4 words: value, max, min, population)
        let words = l.get_attribute(0x0ED0, 4)?; // LEP_RAD_SPOTMETER_OBJ_KELVIN
        let spot_raw = words[0]; // first word is the spotmeter value
        eprintln!("[lepton-getthermal] spot: resolution={resolution}, raw={spot_raw}, words={words:?}");

        // Determine resolution: if raw value > 10000, it's clearly 0.01K encoding
        // (at 0.1K, 10000 raw = 727°C which is unrealistic for normal use)
        let celsius = if spot_raw > 10000 || resolution != 0 {
            // 0.01K resolution: raw value is in centikelvins
            (spot_raw as f64 - 27315.0) / 100.0
        } else {
            // 0.1K resolution: raw value is in decikelvins
            (spot_raw as f64 - 2731.5) / 10.0
        };
        Ok(celsius)
    })
}

/// Sensor gain: "high" (~ -10..140 °C, more sensitive), "low" (~ -10..400 °C)
/// or "auto". Runs an FFC afterwards, as FLIR recommends after a gain change.
#[tauri::command]
pub fn set_gain_mode(state: State<'_, AppState>, mode: String) -> Result<(), String> {
    let m = match mode.as_str() {
        "high" => 0,
        "low" => 1,
        "auto" => 2,
        _ => return Err(format!("Unknown gain mode: {mode}")),
    };
    with_lepton(&state, "set_gain_mode", |l| {
        l.set_gain_mode(m)?;
        l.perform_ffc()
    })
}

#[tauri::command]
pub fn get_gain_mode(state: State<'_, AppState>) -> Result<String, String> {
    with_lepton(&state, "get_gain_mode", |l| {
        Ok(match l.get_gain_mode()? {
            0 => "high",
            1 => "low",
            _ => "auto",
        }
        .to_string())
    })
}

#[derive(Debug, serde::Serialize)]
pub struct Emissivity {
    emissivity: f64,
    reflected_c: f64,
}

/// Scene emissivity and reflected (background) temperature from the Lepton.
#[tauri::command]
pub fn get_emissivity(state: State<'_, AppState>) -> Result<Emissivity, String> {
    with_lepton(&state, "get_emissivity", |l| {
        let p = l.get_flux_params()?;
        Ok(Emissivity {
            emissivity: p[0] as f64 / 8192.0,
            reflected_c: p[1] as f64 / 100.0 - 273.15,
        })
    })
}

/// Set scene emissivity (0.01..1) and reflected temperature (°C). The Lepton
/// applies them to the radiometric pixel values and the spot meter.
#[tauri::command]
pub fn set_emissivity(state: State<'_, AppState>, emissivity: f64, reflected_c: f64) -> Result<(), String> {
    if !(0.01..=1.0).contains(&emissivity) || !(-50.0..=200.0).contains(&reflected_c) {
        return Err(format!("Out of range: emissivity {emissivity}, reflected {reflected_c} °C"));
    }
    with_lepton(&state, "set_emissivity", |l| l.set_emissivity(emissivity, reflected_c))
}
