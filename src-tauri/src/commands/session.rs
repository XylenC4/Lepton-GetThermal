//! Session file: the UI settings of the last session (palette, rotation,
//! measurement tools, ...), restored at the next start.
//!
//! Location: `session.json` next to the executable when that folder is
//! writable (portable use), otherwise in the per-user app config folder.
//! The content is owned by the frontend; Rust only stores the JSON.

use std::path::PathBuf;
use tauri::{AppHandle, Manager};

const FILE_NAME: &str = "session.json";

fn exe_dir_writable() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let probe = dir.join(".session-write-test");
    std::fs::write(&probe, b"").ok()?;
    let _ = std::fs::remove_file(probe);
    Some(dir)
}

fn session_path(app: &AppHandle) -> Result<PathBuf, String> {
    if let Some(dir) = exe_dir_writable() {
        return Ok(dir.join(FILE_NAME));
    }
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir.join(FILE_NAME))
}

/// The saved session, or `null` when there is none (or it can't be read).
#[tauri::command]
pub fn load_session(app: AppHandle) -> Option<serde_json::Value> {
    let path = session_path(&app).ok()?;
    let text = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            eprintln!("[lepton-getthermal] Ignoring unreadable {}: {e}", path.display());
            None
        }
    }
}

/// Writes the session (atomically: temp file + rename). Returns the file path.
#[tauri::command]
pub fn save_session(app: AppHandle, session: serde_json::Value) -> Result<String, String> {
    let path = session_path(&app)?;
    let text = serde_json::to_string_pretty(&session).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path.display().to_string())
}
