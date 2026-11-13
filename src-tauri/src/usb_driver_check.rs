//! Windows: verify which driver is bound to the PureThermal before using libusb.
//!
//! libusb can only open the camera when the whole device (the USB composite
//! parent `USB\VID_1E4E&PID_0100`) is bound to **WinUSB** or **libusbK**. Out
//! of the box Windows binds `usbccgp` (composite) + `usbvideo` (webcam), and
//! libusb then fails with an unhelpful error. This module reads the bound
//! service of every PureThermal device node through SetupAPI and turns it into
//! a clear status + fix-it hint.
//!
//! The classification is plain Rust (unit-tested on every platform); only the
//! enumeration is Windows-specific.

use serde::Serialize;

pub const PT_HWID: &str = "VID_1E4E&PID_0100";

/// One device node as seen by Windows' PnP manager.
#[derive(Debug, Clone, Default)]
pub struct DeviceNode {
    pub instance_id: String,
    pub hardware_ids: Vec<String>,
    /// Bound driver service, e.g. "WinUSB", "libusbK", "usbccgp", "usbvideo".
    pub service: Option<String>,
    /// Device Manager problem code (e.g. 28 = no driver installed), if any.
    pub problem: Option<u32>,
}

impl DeviceNode {
    fn is_pt(&self) -> bool {
        self.hardware_ids.iter().any(|h| h.to_ascii_uppercase().contains(PT_HWID))
            || self.instance_id.to_ascii_uppercase().contains(PT_HWID)
    }
    /// Interface number for `...&MI_xx` child nodes, None for the parent.
    fn interface(&self) -> Option<u8> {
        let all = std::iter::once(&self.instance_id).chain(self.hardware_ids.iter());
        for s in all {
            let up = s.to_ascii_uppercase();
            if let Some(p) = up.find("&MI_") {
                return u8::from_str_radix(up.get(p + 4..p + 6)?, 16).ok();
            }
        }
        None
    }
    fn service_lc(&self) -> String {
        self.service.clone().unwrap_or_default().to_ascii_lowercase()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DriverState {
    /// WinUSB / libusbK on the composite parent: libusb has the whole device.
    Ok,
    /// Usable, but not the recommended setup (e.g. WinUSB only on the video function).
    Warning,
    /// libusb will not be able to stream with this driver setup.
    WrongDriver,
    /// No PureThermal present.
    NotConnected,
    /// Not Windows: no driver check needed.
    #[cfg_attr(windows, allow(dead_code))]
    NotApplicable,
}

#[derive(Debug, Clone, Serialize)]
pub struct DriverReport {
    pub state: DriverState,
    /// Short driver label for the UI, e.g. "WinUSB", "libusbK", "usbvideo (Windows webcam)".
    pub driver: String,
    /// One-line explanation.
    pub summary: String,
    /// What to do about it (empty when OK).
    pub hint: String,
    /// Raw device nodes (instance id -> service), for the log.
    pub details: Vec<String>,
}

impl DriverReport {
    pub fn usable(&self) -> bool {
        matches!(self.state, DriverState::Ok | DriverState::Warning | DriverState::NotApplicable)
    }
    pub fn message(&self) -> String {
        if self.hint.is_empty() {
            self.summary.clone()
        } else {
            format!("{} {}", self.summary, self.hint)
        }
    }
}

const ZADIG_HINT: &str = "Fix: run Zadig (zadig.akeo.ie) -> Options -> List All Devices, untick \
\"Ignore Hubs or Composite Parents\", select \"PureThermal ... (Composite Parent)\" (USB ID 1E4E 0100), \
choose WinUSB (or libusbK) and click Replace Driver. Then replug the camera.";

fn label(service: &str) -> String {
    match service.to_ascii_lowercase().as_str() {
        "winusb" => "WinUSB".into(),
        "libusbk" => "libusbK".into(),
        "libusb0" => "libusb-win32 (libusb0)".into(),
        "usbvideo" => "usbvideo (Windows webcam driver)".into(),
        "usbccgp" => "usbccgp (Windows composite driver)".into(),
        "" => "none".into(),
        other => other.into(),
    }
}

fn is_libusb_driver(s: &str) -> bool {
    s == "winusb" || s == "libusbk"
}

/// Decide whether libusb can use the PureThermal with the drivers Windows has bound.
pub fn classify(nodes: &[DeviceNode]) -> DriverReport {
    let pt: Vec<&DeviceNode> = nodes.iter().filter(|n| n.is_pt()).collect();
    let details: Vec<String> = pt
        .iter()
        .map(|n| {
            format!(
                "{} -> {}{}",
                n.instance_id,
                n.service.as_deref().unwrap_or("(no driver)"),
                n.problem.map(|p| format!(" [problem code {p}]")).unwrap_or_default()
            )
        })
        .collect();
    let report = |state, driver: String, summary: String, hint: &str| DriverReport {
        state,
        driver,
        summary,
        hint: hint.to_string(),
        details: details.clone(),
    };

    let Some(parent) = pt.iter().find(|n| n.interface().is_none()) else {
        return report(
            DriverState::NotConnected,
            "none".into(),
            "PureThermal (USB 1E4E:0100) is not connected.".into(),
            "Plug in the camera (directly, not through a hub if possible).",
        );
    };
    let parent_svc = parent.service_lc();

    if let Some(code) = parent.problem {
        let what = match code {
            28 => "no driver is installed".to_string(),
            10 => "the device cannot start".to_string(),
            43 => "Windows stopped the device (it reported a problem)".to_string(),
            c => format!("Device Manager problem code {c}"),
        };
        return report(
            DriverState::WrongDriver,
            label(&parent_svc),
            format!("PureThermal driver problem: {what}."),
            ZADIG_HINT,
        );
    }

    if is_libusb_driver(&parent_svc) {
        return report(
            DriverState::Ok,
            label(&parent_svc),
            format!("PureThermal uses {} (OK).", label(&parent_svc)),
            "",
        );
    }

    if parent_svc == "libusb0" {
        return report(
            DriverState::WrongDriver,
            label(&parent_svc),
            "PureThermal uses the libusb-win32 (libusb0) driver, which cannot stream isochronous video.".into(),
            ZADIG_HINT,
        );
    }

    if parent_svc == "usbccgp" {
        // Composite driver splits the device: look at the video function (interface 0).
        let vc = pt.iter().find(|n| n.interface() == Some(0));
        let vc_svc = vc.map(|n| n.service_lc()).unwrap_or_default();
        if is_libusb_driver(&vc_svc) {
            return report(
                DriverState::Warning,
                format!("{} (interface 0 only)", label(&vc_svc)),
                format!(
                    "{} is installed on \"Interface 0\" instead of the whole device; streaming may fail \
                     because the video interface (1) is not reachable.",
                    label(&vc_svc)
                ),
                ZADIG_HINT,
            );
        }
        if vc_svc == "usbvideo" {
            return report(
                DriverState::WrongDriver,
                label("usbvideo"),
                "PureThermal is using the Windows webcam driver (usbvideo), which this app cannot use.".into(),
                ZADIG_HINT,
            );
        }
        return report(
            DriverState::WrongDriver,
            format!("usbccgp + {}", label(&vc_svc)),
            format!(
                "PureThermal is split by the Windows composite driver; the video interface uses \"{}\".",
                label(&vc_svc)
            ),
            ZADIG_HINT,
        );
    }

    if parent_svc.is_empty() {
        return report(
            DriverState::WrongDriver,
            "none".into(),
            "PureThermal has no driver installed.".into(),
            ZADIG_HINT,
        );
    }

    report(
        DriverState::WrongDriver,
        label(&parent_svc),
        format!("PureThermal uses an unsupported driver: {}.", label(&parent_svc)),
        ZADIG_HINT,
    )
}

/// Check the driver setup of the attached PureThermal (Windows only).
pub fn check() -> DriverReport {
    #[cfg(windows)]
    {
        match win::enumerate() {
            Ok(nodes) => classify(&nodes),
            Err(e) => DriverReport {
                state: DriverState::Warning,
                driver: "unknown".into(),
                summary: format!("Could not query the USB driver ({e}); trying anyway."),
                hint: String::new(),
                details: vec![],
            },
        }
    }
    #[cfg(not(windows))]
    {
        DriverReport {
            state: DriverState::NotApplicable,
            driver: if cfg!(target_os = "macos") { "IOKit".into() } else { "libusb".into() },
            summary: "No driver check needed on this platform.".into(),
            hint: String::new(),
            details: vec![],
        }
    }
}

#[cfg(windows)]
mod win {
    use super::DeviceNode;
    use windows_sys::Win32::Devices::DeviceAndDriverInstallation::*;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn from_wide(buf: &[u16]) -> String {
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        String::from_utf16_lossy(&buf[..end])
    }

    fn multi_sz(buf: &[u16]) -> Vec<String> {
        buf.split(|&c| c == 0)
            .filter(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    }

    unsafe fn reg_prop(set: HDEVINFO, data: &SP_DEVINFO_DATA, prop: u32) -> Option<Vec<u16>> {
        let mut buf = vec![0u16; 1024];
        let mut needed = 0u32;
        let ok = SetupDiGetDeviceRegistryPropertyW(
            set,
            data,
            prop,
            std::ptr::null_mut(),
            buf.as_mut_ptr() as *mut u8,
            (buf.len() * 2) as u32,
            &mut needed,
        );
        if ok == 0 {
            return None;
        }
        buf.truncate((needed as usize).div_ceil(2));
        Some(buf)
    }

    pub fn enumerate() -> Result<Vec<DeviceNode>, String> {
        let mut nodes = Vec::new();
        unsafe {
            let usb = wide("USB");
            let set = SetupDiGetClassDevsW(
                std::ptr::null(),
                usb.as_ptr(),
                std::ptr::null_mut(),
                DIGCF_PRESENT | DIGCF_ALLCLASSES,
            );
            if set == -1 {
                return Err("SetupDiGetClassDevsW failed".into());
            }
            let mut i = 0u32;
            loop {
                let mut data: SP_DEVINFO_DATA = std::mem::zeroed();
                data.cbSize = std::mem::size_of::<SP_DEVINFO_DATA>() as u32;
                if SetupDiEnumDeviceInfo(set, i, &mut data) == 0 {
                    break;
                }
                i += 1;

                let hardware_ids = reg_prop(set, &data, SPDRP_HARDWAREID)
                    .map(|b| multi_sz(&b))
                    .unwrap_or_default();
                let mut id_buf = vec![0u16; 512];
                let mut needed = 0u32;
                let instance_id = if SetupDiGetDeviceInstanceIdW(
                    set,
                    &data,
                    id_buf.as_mut_ptr(),
                    id_buf.len() as u32,
                    &mut needed,
                ) != 0
                {
                    from_wide(&id_buf)
                } else {
                    String::new()
                };
                let node = DeviceNode {
                    instance_id,
                    hardware_ids,
                    service: reg_prop(set, &data, SPDRP_SERVICE)
                        .map(|b| from_wide(&b))
                        .filter(|s| !s.is_empty()),
                    problem: {
                        let mut status = 0u32;
                        let mut problem = 0u32;
                        if CM_Get_DevNode_Status(&mut status, &mut problem, data.DevInst, 0) == CR_SUCCESS
                            && status & DN_HAS_PROBLEM != 0
                        {
                            Some(problem)
                        } else {
                            None
                        }
                    },
                };
                if node.is_pt() {
                    nodes.push(node);
                }
            }
            SetupDiDestroyDeviceInfoList(set);
        }
        Ok(nodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, svc: Option<&str>) -> DeviceNode {
        DeviceNode {
            instance_id: id.into(),
            hardware_ids: vec![id.split('\\').take(2).collect::<Vec<_>>().join("\\")],
            service: svc.map(Into::into),
            problem: None,
        }
    }
    const PARENT: &str = "USB\\VID_1E4E&PID_0100\\00210030-5109";
    const MI0: &str = "USB\\VID_1E4E&PID_0100&MI_00\\7&abc&0&0000";

    #[test]
    fn winusb_on_parent_is_ok() {
        let r = classify(&[node(PARENT, Some("WinUSB"))]);
        assert_eq!(r.state, DriverState::Ok);
        assert_eq!(r.driver, "WinUSB");
        assert!(r.usable());
    }

    #[test]
    fn libusbk_on_parent_is_ok() {
        let r = classify(&[node(PARENT, Some("libusbK"))]);
        assert_eq!(r.state, DriverState::Ok);
        assert_eq!(r.driver, "libusbK");
    }

    #[test]
    fn stock_windows_webcam_driver_is_rejected() {
        let r = classify(&[node(PARENT, Some("usbccgp")), node(MI0, Some("usbvideo"))]);
        assert_eq!(r.state, DriverState::WrongDriver);
        assert!(!r.usable());
        assert!(r.summary.contains("webcam"));
        assert!(r.hint.contains("Composite Parent"));
    }

    #[test]
    fn winusb_on_interface_only_is_a_warning() {
        let r = classify(&[node(PARENT, Some("usbccgp")), node(MI0, Some("WinUSB"))]);
        assert_eq!(r.state, DriverState::Warning);
        assert!(r.usable());
        assert!(r.driver.contains("interface 0"));
    }

    #[test]
    fn libusb0_is_rejected() {
        let r = classify(&[node(PARENT, Some("libusb0"))]);
        assert_eq!(r.state, DriverState::WrongDriver);
    }

    #[test]
    fn missing_driver_and_problem_codes() {
        let r = classify(&[node(PARENT, None)]);
        assert_eq!(r.state, DriverState::WrongDriver);
        let mut n = node(PARENT, Some("WinUSB"));
        n.problem = Some(28);
        let r = classify(&[n]);
        assert_eq!(r.state, DriverState::WrongDriver);
        assert!(r.summary.contains("no driver"));
    }

    #[test]
    fn not_connected_and_other_devices_ignored() {
        let other = node("USB\\VID_046D&PID_0825\\123", Some("usbvideo"));
        let r = classify(&[other]);
        assert_eq!(r.state, DriverState::NotConnected);
    }

    #[test]
    fn interface_parsing() {
        assert_eq!(node(MI0, None).interface(), Some(0));
        assert_eq!(node(PARENT, None).interface(), None);
    }
}
