use super::*;
use windows::{
    Win32::System::{Com::*, UpdateAgent::*},
    core::{BSTR, Interface},
};
const CRITERIA: &str =
    "IsInstalled=0 and IsHidden=0 and Type='Driver' and IsAssigned=1 and BrowseOnly=0";
fn session<T>(operation: impl FnOnce(&IUpdateSession) -> windows::core::Result<T>) -> Result<T> {
    crate::atlas::preflight::com(|| unsafe {
        let session: IUpdateSession = CoCreateInstance(&UpdateSession, None, CLSCTX_INPROC_SERVER)?;
        session.SetClientApplicationID(&BSTR::from("Ruhtweaks AMD"))?;
        operation(&session)
    })
}
pub(super) fn devices(info: &SystemInfo) -> Result<Vec<Device>> {
    #[derive(Deserialize)]
    struct Pnp {
        #[serde(rename = "PNPDeviceID")]
        instance: String,
        #[serde(rename = "HardwareID")]
        ids: Option<Vec<String>>,
        #[serde(rename = "CompatibleID")]
        compatible: Option<Vec<String>>,
    }
    let wmi = wmi::WMIConnection::new().map_err(|e| fail(e.to_string()))?;
    let rows: Vec<Pnp> = wmi
        .raw_query("SELECT PNPDeviceID,HardwareID,CompatibleID FROM Win32_PnPEntity WHERE PNPClass='Display'")
        .map_err(|e| fail(e.to_string()))?;
    Ok(rows
        .into_iter()
        .filter(|row| {
            info.gpus
                .iter()
                .any(|g| g.hardware_id.eq_ignore_ascii_case(&row.instance))
                && row
                    .instance
                    .to_ascii_uppercase()
                    .starts_with("PCI\\VEN_1002&")
        })
        .map(|row| {
            let mut ids = row.ids.unwrap_or_default();
            ids.extend(row.compatible.unwrap_or_default());
            Device {
                instance: row.instance,
                ids,
            }
        })
        .filter(|d| !d.ids.is_empty())
        .collect())
}
fn candidate(update: &IUpdate) -> windows::core::Result<Candidate> {
    // SAFETY: WUA interfaces are local to this COM apartment and dropped before uninitialize.
    unsafe {
        let driver: IWindowsDriverUpdate = update.cast()?;
        let identity = update.Identity()?;
        Ok(Candidate {
            id: identity.UpdateID()?.to_string(),
            revision: identity.RevisionNumber()?,
            title: update.Title()?.to_string(),
            hardware_id: driver.DriverHardwareID()?.to_string(),
            class: driver.DriverClass()?.to_string(),
            manufacturer: driver.DriverManufacturer()?.to_string(),
            requires_ui: update.InstallationBehavior()?.CanRequestUserInput()?.0 != 0,
        })
    }
}
fn search_native(session: &IUpdateSession) -> windows::core::Result<IUpdateCollection> {
    unsafe {
        let result = session
            .CreateUpdateSearcher()?
            .Search(&BSTR::from(CRITERIA))?;
        if result.ResultCode()? != orcSucceeded {
            return Err(windows::core::Error::from_hresult(windows::core::HRESULT(
                0x80240022_u32 as i32,
            )));
        }
        result.Updates()
    }
}
fn installed_native(
    session: &IUpdateSession,
    requested: &[Candidate],
) -> windows::core::Result<bool> {
    unsafe {
        let searcher = session.CreateUpdateSearcher()?;
        for update in requested {
            if !valid_id(&update.id) || update.revision < 0 {
                return Ok(false);
            }
            let query = BSTR::from(format!(
                "IsInstalled=1 and Type='Driver' and UpdateID='{}' and RevisionNumber={}",
                update.id, update.revision
            ));
            let result = searcher.Search(&query)?;
            if result.ResultCode()? != orcSucceeded || result.Updates()?.Count()? == 0 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
pub(super) fn installed(requested: &[Candidate]) -> Result<bool> {
    session(|s| installed_native(s, requested))
}
pub(super) struct Native<'a> {
    pub store: &'a Store,
    pub profile: &'a Profile,
    pub other_gpus: bool,
}
impl Backend for Native<'_> {
    fn search(&mut self) -> Result<Vec<Candidate>> {
        session(|s| unsafe {
            let updates = search_native(s)?;
            let mut result = Vec::new();
            for i in 0..updates.Count()? {
                result.push(candidate(&updates.get_Item(i)?)?);
            }
            Ok(result)
        })
    }
    fn installed(&mut self, requested: &[Candidate]) -> Result<bool> {
        installed(requested)
    }
    fn install(&mut self, updates: &[Candidate]) -> Result<u32> {
        let request = Request {
            schema_version: 1,
            profile_hash: self.profile.fingerprint()?,
            created_ms: now_ms()?,
            parent_pid: std::process::id(),
            binary_hash: crate::security::digest_file(&std::env::current_exe()?)?,
            boot_before: crate::atlas::preflight::boot_id()?,
            updates: updates.to_vec(),
            needs_reboot: false,
            other_gpus: self.other_gpus,
        };
        let folder = self.store.root.join("amd-jobs");
        std::fs::create_dir_all(&folder)?;
        let temporary = tempfile::Builder::new()
            .prefix("amd-")
            .suffix(".json")
            .tempfile_in(&folder)?;
        let (_, path) = temporary.keep().map_err(|e| e.error)?;
        atomic_json(&path, &request)?;
        atomic_json(&self.store.root.join("amd-pending.json"), &request)?;
        let path = path.canonicalize()?;
        let digest = crate::security::digest_file(&path)?;
        let params = format!("--amd-update-helper \"{}\" {digest}", path.display());
        let code = crate::security::launch::wait_elevated_self(&params)?.ok_or_else(|| {
            fail("Windows não retornou handle do instalador AMD; revalide antes de repetir.")
        })?;
        if code == 3010 {
            let mut request = request;
            request.needs_reboot = true;
            atomic_json(&self.store.root.join("amd-pending.json"), &request)?;
        }
        if code == 0 && installed(updates)? {
            std::fs::remove_file(path)?;
        }
        Ok(code)
    }
}
pub(super) fn install(requested: &[Candidate]) -> Result<u32> {
    session(|s| {
        let outcome = (|| -> windows::core::Result<u32> {
            unsafe {
                let offers = search_native(s)?;
                for requested in requested {
                    let mut selected = None;
                    for i in 0..offers.Count()? {
                        let update = offers.get_Item(i)?;
                        let actual = candidate(&update)?;
                        if actual.id.eq_ignore_ascii_case(&requested.id)
                            && actual.revision == requested.revision
                            && actual
                                .hardware_id
                                .eq_ignore_ascii_case(&requested.hardware_id)
                            && actual.class.eq_ignore_ascii_case("Display")
                            && !actual.requires_ui
                            && actual.manufacturer == requested.manufacturer
                        {
                            selected = Some(update);
                            break;
                        }
                    }
                    let Some(update) = selected else {
                        if installed_native(s, std::slice::from_ref(requested))? {
                            continue;
                        }
                        return Ok(0x80240017);
                    };
                    if update.EulaAccepted()?.0 == 0 {
                        update.AcceptEula()?;
                    }
                    let collection: IUpdateCollection =
                        CoCreateInstance(&UpdateCollection, None, CLSCTX_INPROC_SERVER)?;
                    collection.Add(&update)?;
                    if update.IsDownloaded()?.0 == 0 {
                        let downloader = s.CreateUpdateDownloader()?;
                        downloader.SetUpdates(&collection)?;
                        let result = downloader.Download()?;
                        if result.ResultCode()? != orcSucceeded
                            || result.GetUpdateResult(0)?.ResultCode()? != orcSucceeded
                        {
                            return Ok(0x80240022);
                        }
                    }
                    let installer = s.CreateUpdateInstaller()?;
                    if installer.RebootRequiredBeforeInstallation()?.0 != 0 {
                        return Ok(3010);
                    }
                    installer.SetAllowSourcePrompts(false.into())?;
                    let quiet: IUpdateInstaller2 = installer.cast()?;
                    quiet.SetForceQuiet(true.into())?;
                    installer.SetUpdates(&collection)?;
                    let result = installer.Install()?;
                    if result.ResultCode()? != orcSucceeded
                        || result.GetUpdateResult(0)?.ResultCode()? != orcSucceeded
                    {
                        return Ok(0x80240022);
                    }
                    if result.RebootRequired()?.0 != 0 {
                        return Ok(3010);
                    }
                }
                // Parent verifies installed identities after this process exits.
                Ok(0)
            }
        })();
        Ok(outcome.unwrap_or_else(|error| error.code().0 as u32))
    })
}
pub(super) fn validate_parent(pid: u32, digest: &str) -> Result<()> {
    use windows_sys::Win32::{Foundation::*, Security::*, System::Threading::*};
    struct Handle(HANDLE);
    impl Drop for Handle {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    // SAFETY: all tokens/process handles are scoped. Query only; no remote writes/injection.
    unsafe {
        let mut token = std::ptr::null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let token = Handle(token);
        let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
        let mut size = 0;
        if GetTokenInformation(
            token.0,
            TokenElevation,
            &mut elevation as *mut _ as *mut _,
            std::mem::size_of::<TOKEN_ELEVATION>() as u32,
            &mut size,
        ) == 0
            || elevation.TokenIsElevated == 0
        {
            return Err(fail("UAC necessário para instalar o driver AMD."));
        }
        let parent = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if parent.is_null() {
            return Err(fail("Aplicativo autorizador não está disponível"));
        }
        let parent = Handle(parent);
        let mut exit = 0;
        if GetExitCodeProcess(parent.0, &mut exit) == 0 || exit != 259 {
            return Err(fail("Aplicativo autorizador encerrado"));
        }
        let mut name = vec![0u16; 32768];
        let mut length = name.len() as u32;
        if QueryFullProcessImageNameW(parent.0, 0, name.as_mut_ptr(), &mut length) == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let parent_path =
            std::path::PathBuf::from(String::from_utf16_lossy(&name[..length as usize]));
        let own = std::env::current_exe()?.canonicalize()?;
        if own != parent_path.canonicalize()?
            || crate::security::digest_file(&own)? != crate::security::validate_digest(digest)?
        {
            return Err(fail("Identidade do aplicativo autorizador não confere"));
        }
    }
    Ok(())
}
