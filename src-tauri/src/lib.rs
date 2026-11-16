#[cfg(target_os = "android")]
mod android_usb;
mod camera;
mod commands;
mod processing;
mod usb_driver_check;
mod usb_stream;
mod uvc_descriptors;
mod uvc_payload;

use camera::acquisition::CameraAcquisition;
use camera::lepton::LeptonController;
use parking_lot::Mutex;
use std::sync::Arc;
use tauri::Manager;

pub struct AppState {
    pub camera: Mutex<Option<CameraAcquisition>>,
    pub lepton: Mutex<Option<Arc<LeptonController>>>,
}

/// Windows: if the folder containing the executable is writable (portable
/// use, e.g. unzipped to a USB stick), keep WebView2's cache in
/// `<exe dir>/webview-data`. Installed copies (Program Files, read-only)
/// fall back to the default per-user location.
fn portable_data_dir() -> Option<std::path::PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let dir = std::env::current_exe().ok()?.parent()?.join("webview-data");
    std::fs::create_dir_all(&dir).ok()?;
    let probe = dir.join(".write-test");
    std::fs::write(&probe, b"").ok()?;
    let _ = std::fs::remove_file(probe);
    Some(dir)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // The main window is created here instead of from tauri.conf.json
            // ("create": false) so a portable build can keep its WebView data
            // next to the executable instead of in the user profile.
            let cfg = app.config().app.windows.first().cloned().expect("window config");
            #[allow(unused_mut)]
            let mut builder = tauri::WebviewWindowBuilder::from_config(app.handle(), &cfg)?;
            if let Some(dir) = portable_data_dir() {
                builder = builder.data_directory(dir);
            }
            builder.build()?;
            Ok(())
        })
        .manage(AppState {
            camera: Mutex::new(None),
            lepton: Mutex::new(None),
        })
        .invoke_handler(tauri::generate_handler![
            commands::stream::connect_camera,
            commands::stream::get_usb_driver_status,
            commands::stream::camera_status,
            commands::stream::disconnect_camera,
            commands::stream::start_stream,
            commands::stream::stop_stream,
            commands::stream::set_palette,
            commands::stream::set_polarity,
            commands::stream::set_isotherm,
            commands::stream::set_display_range,
            commands::stream::record_reference,
            commands::stream::clear_reference,
            commands::stream::set_diff_mode,
            commands::stream::set_contrast_mode,
            commands::stream::set_hold_mode,
            commands::stream::reset_hold,
            commands::stream::get_reference,
            commands::stream::set_upscale,
            commands::stream::set_rotation,
            commands::session::load_session,
            commands::session::save_session,
            commands::controls::perform_ffc,
            commands::controls::get_polarity,
            commands::controls::get_device_info,
            commands::controls::get_spotmeter_roi,
            commands::controls::set_spotmeter_roi,
            commands::controls::get_spot_temperature,
            commands::controls::set_gain_mode,
            commands::controls::get_gain_mode,
            commands::controls::get_emissivity,
            commands::controls::set_emissivity,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Close the camera explicitly on exit: dropping the last UsbStream
            // stops streaming and hands the device back to the OS driver
            // (Linux: uvcvideo is re-attached, /dev/video* comes back).
            if let tauri::RunEvent::Exit = event {
                if let Some(state) = app.try_state::<AppState>() {
                    state.camera.lock().take();
                    state.lepton.lock().take();
                }
            }
        });
}
