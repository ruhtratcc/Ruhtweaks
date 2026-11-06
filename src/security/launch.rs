use crate::{Result, fail, security};
use std::path::Path;

/// A normal Windows Shell launch: retains MOTW, security dialogs and UAC.
/// Only reviewed arguments reach here. External installers use normal `open`.
pub fn wait(path: &Path, digest: &str, source: &str, parameters: &str) -> Result<Option<u32>> {
    security::verify_pe_header(path)?;
    if security::digest_file(path)? != security::validate_digest(digest)? {
        return Err(fail("Arquivo preparado foi substituído"));
    }
    security::preserve_download_protection(path, source)?;
    let path = path.to_path_buf();
    let digest = digest.to_string();
    let parameters = parameters.to_string();
    std::thread::Builder::new()
        .name("ruhtweaks-installer".into())
        .spawn(move || native_wait(&path, &digest, &parameters, "open"))?
        .join()
        .map_err(|_| fail("Falha na thread do instalador; revalide antes de repetir"))?
}
#[cfg(windows)]
fn native_wait(path: &Path, digest: &str, parameters: &str, verb: &str) -> Result<Option<u32>> {
    use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt};
    use windows::Win32::System::Com::*;
    use windows_sys::Win32::{
        Foundation::*,
        System::Threading::*,
        UI::{Shell::*, WindowsAndMessaging::SW_SHOWNORMAL},
    };
    let path = path.canonicalize()?;
    let _lock = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&path)?;
    if security::digest_file(&path)? != security::validate_digest(digest)? {
        return Err(fail("Hash mudou antes da execução"));
    }
    let text = path
        .to_str()
        .ok_or_else(|| fail("Caminho de instalação não representável"))?;
    let text = text.strip_prefix("\\\\?\\").unwrap_or(text);
    if text.starts_with("UNC\\") || text.starts_with("\\\\") {
        return Err(fail("Execução de instalador em caminho de rede bloqueada"));
    }
    let file: Vec<u16> = std::ffi::OsStr::new(text)
        .encode_wide()
        .chain(Some(0))
        .collect();
    let args: Vec<u16> = parameters.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = verb.encode_utf16().chain(Some(0)).collect();
    struct Apartment;
    impl Drop for Apartment {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    // SAFETY: this dedicated thread owns an STA; buffers and locked file outlive
    // ShellExecuteEx and process wait. Every returned handle is closed by RAII.
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE)
            .ok()
            .map_err(|e| fail(e.to_string()))?;
        let _apartment = Apartment;
        let mut info: SHELLEXECUTEINFOW = std::mem::zeroed();
        info.cbSize = std::mem::size_of_val(&info) as u32;
        info.fMask = SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC;
        info.lpVerb = verb.as_ptr();
        info.lpFile = file.as_ptr();
        info.lpParameters = args.as_ptr();
        info.nShow = SW_SHOWNORMAL;
        if ShellExecuteExW(&mut info) == 0 {
            let code = GetLastError();
            if code == ERROR_CANCELLED {
                return Ok(Some(1223));
            }
            return Err(fail(format!(
                "Windows recusou abrir o instalador ({code}). Nenhuma proteção será contornada."
            )));
        }
        if info.hProcess.is_null() {
            return Ok(None);
        }
        struct Handle(HANDLE);
        impl Drop for Handle {
            fn drop(&mut self) {
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
        let handle = Handle(info.hProcess);
        if WaitForSingleObject(handle.0, INFINITE) != WAIT_OBJECT_0 {
            return Err(fail("Espera do instalador não confirmada"));
        }
        let mut exit = 0;
        if GetExitCodeProcess(handle.0, &mut exit) == 0 {
            return Err(fail("Código de saída indisponível"));
        }
        Ok(Some(exit))
    }
}
#[cfg(not(windows))]
fn native_wait(_: &Path, _: &str, _: &str, _: &str) -> Result<Option<u32>> {
    Err(fail("Instalação requer Windows"))
}
/// Elevate this exact executable for the narrowly scoped AMD WUA operation.
/// Windows displays its normal UAC; the helper exits after the operation.
pub fn wait_elevated_self(parameters: &str) -> Result<Option<u32>> {
    let path = std::env::current_exe()?;
    let digest = security::digest_file(&path)?;
    let parameters = parameters.to_owned();
    std::thread::spawn(move || native_wait(&path, &digest, &parameters, "runas"))
        .join()
        .map_err(|_| fail("Falha na operação AMD; revalide antes de repetir"))?
}
