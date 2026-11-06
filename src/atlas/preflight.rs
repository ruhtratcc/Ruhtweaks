use crate::{Result, fail};
#[cfg(windows)]
pub fn ac_power() -> Result<bool> {
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut status: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
    // SAFETY: Windows fills a live correctly sized POD. This only reads power status.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(status.ACLineStatus == 1 || status.BatteryFlag == 128)
}
#[cfg(not(windows))]
pub fn ac_power() -> Result<bool> {
    Err(fail("Requer Windows"))
}
#[cfg(windows)]
pub(crate) fn com<T>(operation: impl FnOnce() -> windows::core::Result<T>) -> Result<T> {
    use windows::Win32::System::Com::*;
    struct Apartment(bool);
    impl Drop for Apartment {
        fn drop(&mut self) {
            if self.0 {
                unsafe {
                    CoUninitialize();
                }
            }
        }
    }
    // SAFETY: COM is scoped to this thread and the interfaces drop inside operation.
    let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    if result.is_err() && result.0 as u32 != 0x80010106 {
        return Err(fail(format!("Consulta COM indisponível: {result:?}")));
    }
    let _apartment = Apartment(result.is_ok());
    operation().map_err(|e| fail(e.to_string()))
}
#[cfg(windows)]
pub fn reboot_pending() -> Result<bool> {
    use windows::Win32::System::{Com::*, UpdateAgent::*};
    com(|| unsafe {
        let info: ISystemInformation =
            CoCreateInstance(&SystemInformation, None, CLSCTX_INPROC_SERVER)?;
        Ok(info.RebootRequired()?.0 != 0)
    })
}
#[cfg(windows)]
pub fn boot_id() -> Result<String> {
    let c = wmi::WMIConnection::new().map_err(|e| fail(e.to_string()))?;
    let rows: Vec<std::collections::HashMap<String, wmi::Variant>> = c
        .raw_query("SELECT LastBootUpTime FROM Win32_OperatingSystem")
        .map_err(|e| fail(e.to_string()))?;
    match rows.first().and_then(|r| r.get("LastBootUpTime")) {
        Some(wmi::Variant::String(s)) if !s.is_empty() => Ok(s.clone()),
        _ => Err(fail("Último boot não identificado")),
    }
}
#[cfg(windows)]
pub fn protections_active() -> Result<bool> {
    use windows::Win32::{NetworkManagement::WindowsFirewall::*, System::Com::*};
    let firewall = com(|| unsafe {
        let policy: INetFwPolicy2 = CoCreateInstance(&NetFwPolicy2, None, CLSCTX_INPROC_SERVER)?;
        let profiles = policy.CurrentProfileTypes()?;
        for profile in [
            NET_FW_PROFILE2_DOMAIN,
            NET_FW_PROFILE2_PRIVATE,
            NET_FW_PROFILE2_PUBLIC,
        ] {
            if profiles & profile.0 != 0 && policy.get_FirewallEnabled(profile)?.0 == 0 {
                return Ok(false);
            }
        }
        Ok(profiles != 0)
    })?;
    let c = wmi::WMIConnection::with_namespace_path("ROOT\\Microsoft\\Windows\\Defender")
        .map_err(|e| fail(e.to_string()))?;
    let rows: Vec<std::collections::HashMap<String, wmi::Variant>> = c.raw_query("SELECT AMServiceEnabled,AntivirusEnabled,RealTimeProtectionEnabled FROM MSFT_MpComputerStatus").map_err(|e| fail(e.to_string()))?;
    let protected = firewall
        && rows.first().is_some_and(|r| {
            [
                "AMServiceEnabled",
                "AntivirusEnabled",
                "RealTimeProtectionEnabled",
            ]
            .iter()
            .all(|k| matches!(r.get(*k), Some(wmi::Variant::Bool(true))))
        });
    let system = wmi::WMIConnection::new().map_err(|e| fail(e.to_string()))?;
    let drivers: Vec<std::collections::HashMap<String, wmi::Variant>> = system
        .raw_query("SELECT State,StartMode FROM Win32_SystemDriver WHERE Name='UCPD'")
        .map_err(|e| fail(e.to_string()))?;
    let ucpd = drivers.first().is_some_and(|r| {
        matches!(r.get("State"), Some(wmi::Variant::String(v)) if v == "Running")
            && matches!(r.get("StartMode"), Some(wmi::Variant::String(v)) if v != "Disabled")
    });
    Ok(protected && ucpd)
}
#[cfg(windows)]
pub fn workspace_space(path: &std::path::Path) -> Result<u64> {
    use std::os::windows::ffi::OsStrExt;
    let absolute = std::path::absolute(path)?;
    let wide: Vec<u16> = absolute.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut free = 0;
    // SAFETY: null-terminated path and writable u64 live through the read-only API.
    if unsafe {
        windows_sys::Win32::Storage::FileSystem::GetDiskFreeSpaceExW(
            wide.as_ptr(),
            &mut free,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(free)
}
#[cfg(not(windows))]
pub fn reboot_pending() -> Result<bool> {
    Err(fail("Requer Windows"))
}
#[cfg(not(windows))]
pub fn boot_id() -> Result<String> {
    Err(fail("Requer Windows"))
}
#[cfg(not(windows))]
pub fn protections_active() -> Result<bool> {
    Err(fail("Requer Windows"))
}
#[cfg(not(windows))]
pub fn workspace_space(_: &std::path::Path) -> Result<u64> {
    Err(fail("Requer Windows"))
}
