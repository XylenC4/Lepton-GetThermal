// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Redirect stderr to a log file so we can see logs when launched via `open`
    #[cfg(all(debug_assertions, unix))]
    {
        use std::fs::File;
        use std::os::unix::io::IntoRawFd;
        if let Ok(f) = File::create(std::env::temp_dir().join("lepton-getthermal.log")) {
            let fd = f.into_raw_fd();
            unsafe { libc::dup2(fd, 2); }
        }
    }

    // Windows GUI builds have no console, so all eprintln!/panic output would be
    // lost. Send stderr to lepton-getthermal.log next to the exe (portable use),
    // or to %TEMP% if that folder is not writable.
    #[cfg(windows)]
    windows_log::redirect_stderr();

    eprintln!(
        "[lepton-getthermal] v{} starting ({} {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );
    lepton_getthermal_lib::run()
}

#[cfg(windows)]
mod windows_log {
    use std::fs::File;
    use std::os::windows::io::IntoRawHandle;
    use windows_sys::Win32::System::Console::{SetStdHandle, STD_ERROR_HANDLE};

    pub fn redirect_stderr() {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()));
        let candidates = exe_dir
            .into_iter()
            .chain(std::iter::once(std::env::temp_dir()))
            .map(|d| d.join("lepton-getthermal.log"));
        for path in candidates {
            if let Ok(f) = File::create(&path) {
                let handle = f.into_raw_handle();
                unsafe { SetStdHandle(STD_ERROR_HANDLE, handle as _) };
                std::panic::set_hook(Box::new(|info| {
                    eprintln!("[lepton-getthermal] PANIC: {info}");
                }));
                return;
            }
        }
    }
}
