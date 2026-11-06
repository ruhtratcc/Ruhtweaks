use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Gpu {
    pub name: String,
    pub hardware_id: String,
    pub driver: String,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Monitor {
    pub name: String,
    pub current_hz: u32,
    pub max_hz: u32,
    pub width: u32,
    pub height: u32,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SystemInfo {
    pub os: String,
    pub build: u32,
    pub edition: String,
    pub architecture: String,
    pub cpu: String,
    pub ram_bytes: u64,
    pub gpus: Vec<Gpu>,
    pub disks: Vec<String>,
    pub network: Vec<String>,
    pub monitors: Vec<Monitor>,
    pub laptop: bool,
    pub warnings: Vec<String>,
}
impl SystemInfo {
    pub fn supported(&self) -> bool {
        cfg!(windows) && self.build >= 17763
    }
    pub fn summary(&self) -> String {
        format!(
            "{} · build {} · {} · {}\n\nCPU  {}\nRAM  {:.1} GB\nGPU  {}\n\nUNIDADES\n{}\n\nREDE\n{}\n\nMONITORES\n{}\n\n{}",
            self.os,
            self.build,
            self.edition,
            self.architecture,
            self.cpu,
            self.ram_bytes as f64 / 1_073_741_824.,
            self.gpus
                .iter()
                .map(|g| format!("{} · driver {}\n{}", g.name, g.driver, g.hardware_id))
                .collect::<Vec<_>>()
                .join("\n"),
            self.disks.join("\n"),
            self.network.join("\n"),
            self.monitors
                .iter()
                .map(|m| format!(
                    "{} · {}×{} · {} Hz · máximo no modo atual: {} Hz",
                    m.name, m.width, m.height, m.current_hz, m.max_hz
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            self.warnings.join("\n")
        )
    }
}
#[cfg(windows)]
pub fn collect() -> SystemInfo {
    use std::collections::HashMap;
    use wmi::{Variant, WMIConnection};
    let mut info = SystemInfo {
        architecture: std::env::consts::ARCH.into(),
        ..Default::default()
    };
    // SAFETY: the output structure has the exact ABI required by GetNativeSystemInfo.
    unsafe {
        let mut native: windows_sys::Win32::System::SystemInformation::SYSTEM_INFO =
            std::mem::zeroed();
        windows_sys::Win32::System::SystemInformation::GetNativeSystemInfo(&mut native);
        info.architecture = match native.Anonymous.Anonymous.wProcessorArchitecture {
            9 => "x86_64",
            12 => "aarch64",
            0 => "x86",
            _ => "desconhecida",
        }
        .into();
    }
    if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
    {
        info.edition = key.get_value("EditionID").unwrap_or_default();
        info.os = key.get_value("ProductName").unwrap_or_default();
        info.build = key
            .get_value::<String, _>("CurrentBuildNumber")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
    }
    info.warnings.push(match crate::security::winget_path() {
        Ok(_) => "WinGet: App Installer registrado e assinatura validada.".into(),
        Err(e) => format!("Pré-requisito WinGet: {e}"),
    });
    let connection = match WMIConnection::new() {
        Ok(c) => c,
        Err(e) => {
            info.warnings.push(format!("WMI indisponível: {e}"));
            return info;
        }
    };
    let mut query = |sql: &str| -> Vec<HashMap<String, Variant>> {
        match connection.raw_query(sql) {
            Ok(r) => r,
            Err(e) => {
                info.warnings.push(format!("Inventário parcial: {e}"));
                vec![]
            }
        }
    };
    fn value(row: &HashMap<String, Variant>, key: &str) -> String {
        match row.get(key) {
            Some(Variant::String(v)) => v.clone(),
            Some(Variant::UI4(v)) => v.to_string(),
            Some(Variant::UI8(v)) => v.to_string(),
            Some(Variant::I4(v)) => v.to_string(),
            _ => String::new(),
        }
    }
    if let Some(row) =
        query("SELECT Caption,BuildNumber,OSArchitecture FROM Win32_OperatingSystem").first()
    {
        info.os = value(row, "Caption");
        info.build = value(row, "BuildNumber").parse().unwrap_or(0);
        let arch = value(row, "OSArchitecture");
        if !arch.is_empty() {
            info.architecture = format!("{} / {}", info.architecture, arch)
        }
    }
    info.cpu = query("SELECT Name FROM Win32_Processor")
        .iter()
        .map(|r| value(r, "Name"))
        .collect::<Vec<_>>()
        .join(" / ");
    if let Some(row) =
        query("SELECT TotalPhysicalMemory,PCSystemType FROM Win32_ComputerSystem").first()
    {
        info.ram_bytes = value(row, "TotalPhysicalMemory").parse().unwrap_or(0);
        info.laptop = value(row, "PCSystemType") == "2";
    }
    info.gpus = query("SELECT Name,PNPDeviceID,DriverVersion FROM Win32_VideoController")
        .iter()
        .map(|r| Gpu {
            name: value(r, "Name"),
            hardware_id: value(r, "PNPDeviceID"),
            driver: value(r, "DriverVersion"),
        })
        .collect();
    info.disks = query("SELECT Model,Size,InterfaceType FROM Win32_DiskDrive")
        .iter()
        .map(|r| {
            format!(
                "{} · {:.1} GB · {}",
                value(r, "Model"),
                value(r, "Size").parse::<f64>().unwrap_or(0.) / 1e9,
                value(r, "InterfaceType")
            )
        })
        .collect();
    info.network = query("SELECT Name FROM Win32_NetworkAdapter WHERE NetEnabled = TRUE")
        .iter()
        .map(|r| value(r, "Name"))
        .collect();
    if let Ok(key) = winreg::RegKey::predef(winreg::enums::HKEY_LOCAL_MACHINE)
        .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion")
    {
        info.edition = key.get_value("EditionID").unwrap_or_default();
    }
    info.monitors = monitors();
    if info.monitors.is_empty() {
        info.warnings
            .push("Nenhum modo de monitor disponível nesta sessão.".into())
    }
    info
}
#[cfg(not(windows))]
pub fn collect() -> SystemInfo {
    SystemInfo {
        os: std::env::consts::OS.into(),
        warnings: vec!["Inventário completo requer Windows 10/11.".into()],
        ..Default::default()
    }
}

#[cfg(windows)]
fn monitors() -> Vec<Monitor> {
    use windows_sys::Win32::Graphics::Gdi::*;
    let mut result = vec![];
    for index in 0..32 {
        // SAFETY: Windows receives correctly sized, initialized output buffers.
        unsafe {
            let mut device: DISPLAY_DEVICEW = std::mem::zeroed();
            device.cb = std::mem::size_of_val(&device) as u32;
            if EnumDisplayDevicesW(std::ptr::null(), index, &mut device, 0) == 0 {
                break;
            }
            if device.StateFlags & DISPLAY_DEVICE_ATTACHED_TO_DESKTOP == 0
                || device.StateFlags & DISPLAY_DEVICE_MIRRORING_DRIVER != 0
            {
                continue;
            }
            let mut mode: DEVMODEW = std::mem::zeroed();
            mode.dmSize = std::mem::size_of_val(&mode) as u16;
            if EnumDisplaySettingsW(device.DeviceName.as_ptr(), ENUM_CURRENT_SETTINGS, &mut mode)
                == 0
            {
                continue;
            }
            let mut monitor = Monitor {
                name: String::from_utf16_lossy(
                    &device.DeviceString[..device
                        .DeviceString
                        .iter()
                        .position(|c| *c == 0)
                        .unwrap_or(device.DeviceString.len())],
                ),
                current_hz: mode.dmDisplayFrequency,
                max_hz: mode.dmDisplayFrequency,
                width: mode.dmPelsWidth,
                height: mode.dmPelsHeight,
            };
            for n in 0..4096 {
                let mut candidate: DEVMODEW = std::mem::zeroed();
                candidate.dmSize = std::mem::size_of_val(&candidate) as u16;
                if EnumDisplaySettingsW(device.DeviceName.as_ptr(), n, &mut candidate) == 0 {
                    break;
                }
                if candidate.dmPelsWidth == mode.dmPelsWidth
                    && candidate.dmPelsHeight == mode.dmPelsHeight
                    && candidate.dmBitsPerPel == mode.dmBitsPerPel
                {
                    monitor.max_hz = monitor.max_hz.max(candidate.dmDisplayFrequency)
                }
            }
            result.push(monitor);
        }
    }
    result
}
