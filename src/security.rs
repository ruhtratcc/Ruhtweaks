use crate::{Result, fail};
pub mod launch;
pub mod transfer;
pub fn verify_pe_header(path: &Path) -> Result<()> {
    use std::io::{Seek, SeekFrom};
    let mut file = File::open(path)?;
    let mut dos = [0; 64];
    file.read_exact(&mut dos)?;
    let offset = u32::from_le_bytes(
        dos[60..64]
            .try_into()
            .map_err(|_| fail("Cabeçalho PE inválido"))?,
    ) as u64;
    if &dos[..2] != b"MZ"
        || !(64..=1024 * 1024).contains(&offset)
        || file.metadata()?.len() < offset + 24
    {
        return Err(fail(
            "Executável ausente, incompleto ou sem cabeçalho PE válido",
        ));
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut signature = [0; 4];
    file.read_exact(&mut signature)?;
    if signature != *b"PE\0\0" {
        return Err(fail("Assinatura de formato PE inválida"));
    }
    Ok(())
}
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};

pub fn client() -> Result<reqwest::blocking::Client> {
    Ok(reqwest::blocking::Client::builder()
        .https_only(true)
        .user_agent(concat!(
            "Ruhtweaks/",
            env!("CARGO_PKG_VERSION"),
            " (no telemetry)"
        ))
        .timeout(Duration::from_secs(300))
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !trusted_network_url(attempt.url()) {
                attempt.error("Redirecionamento não autorizado")
            } else {
                attempt.follow()
            }
        }))
        .build()?)
}
fn trusted_network_url(url: &reqwest::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "api.github.com"
                    | "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
        && url.port_or_known_default() == Some(443)
}
pub fn digest_file(path: &Path) -> Result<String> {
    let mut f = File::open(path)?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let n = f.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
pub fn validate_digest(digest: &str) -> Result<&str> {
    let hash = digest
        .strip_prefix("sha256:")
        .ok_or_else(|| fail("Release sem digest SHA-256 verificável; use o site oficial."))?;
    if hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err(fail("SHA-256 inválido"));
    }
    Ok(hash)
}
pub fn download_verified(
    url: &str,
    digest: &str,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf> {
    download_with_client(&client()?, url, digest, destination, cancel, &mut progress)
}
pub fn download_with_client(
    client: &reqwest::blocking::Client,
    url: &str,
    digest: &str,
    destination: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf> {
    let parsed = reqwest::Url::parse(url).map_err(|_| fail("URL inválida"))?;
    if !trusted_network_url(&parsed)
        || parsed.host_str() != Some("github.com")
        || !(parsed
            .path()
            .starts_with("/Ak4ai/Litecord/releases/download/")
            || parsed
                .path()
                .starts_with("/Atlas-OS/Atlas/releases/download/")
            || parsed
                .path()
                .starts_with("/Ameliorated-LLC/trusted-uninstaller-cli/releases/download/"))
    {
        return Err(fail("Origem de download não autorizada"));
    }
    let expected = validate_digest(digest)?;
    let parent = destination
        .parent()
        .ok_or_else(|| fail("Destino inválido"))?;
    std::fs::create_dir_all(parent)?;
    transfer::recover(parent, digest)?;
    if destination.exists() && digest_file(destination)? == expected.to_ascii_lowercase() {
        if destination.extension().is_some_and(|e| e == "exe") {
            preserve_download_protection(destination, url)?;
            if digest_file(destination)? != expected.to_ascii_lowercase() {
                return Err(fail("Arquivo mudou após inspeção de segurança"));
            }
        }
        return Ok(destination.to_path_buf());
    }
    let mut response = client.get(parsed).send()?.error_for_status()?;
    let total = response.content_length();
    transfer::copy_verified(
        &mut response,
        destination,
        digest,
        total,
        cancel,
        &mut progress,
    )?;
    if destination.extension().is_some_and(|e| e == "exe") {
        preserve_download_protection(destination, url)?;
        if digest_file(destination)? != expected.to_ascii_lowercase() {
            return Err(fail(
                "Arquivo mudou após inspeção de segurança. Não será executado.",
            ));
        }
    }
    Ok(destination.to_path_buf())
}
#[cfg(windows)]
pub fn preserve_download_protection(path: &Path, source: &str) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use windows::{
        Win32::{
            System::Com::*,
            UI::Shell::{AttachmentServices, IAttachmentExecute},
        },
        core::PCWSTR,
    };
    let url = reqwest::Url::parse(source).map_err(|_| fail("Origem inválida"))?;
    if !trusted_network_url(&url) && !crate::drivers::amd::allowed(&url) {
        return Err(fail("Origem de anexo não autorizada"));
    }
    let path = std::path::absolute(path)?;
    let local: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let source: Vec<u16> = source.encode_utf16().chain(Some(0)).collect();
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
    // SAFETY: COM is initialized on this thread, interfaces drop before uninitialization,
    // and both UTF-16 buffers remain alive. Save preserves Windows attachment policy.
    let result = unsafe {
        let initialized = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if initialized.is_err() && initialized.0 as u32 != 0x80010106 {
            return Err(fail(format!(
                "Attachment Services indisponível: {initialized:?}"
            )));
        }
        let _apartment = Apartment(initialized.is_ok());
        (|| -> windows::core::Result<()> {
            let attachment: IAttachmentExecute =
                CoCreateInstance(&AttachmentServices, None, CLSCTX_INPROC_SERVER)?;
            attachment.SetLocalPath(PCWSTR(local.as_ptr()))?;
            attachment.SetSource(PCWSTR(source.as_ptr()))?;
            attachment.Save()
        })()
    };
    result.map_err(|e| {
        fail(format!(
            "Windows bloqueou ou não confirmou a inspeção do download: {e}"
        ))
    })
}
#[cfg(not(windows))]
pub fn preserve_download_protection(_path: &Path, _source: &str) -> Result<()> {
    Err(fail("Attachment Services requer Windows"))
}

#[cfg(windows)]
pub fn verify_signature(path: &Path) -> Result<()> {
    verify_publisher(path, &[]).map(|_| ())
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Signature {
    Valid(String),
    Unsigned,
    Unverifiable(u32),
}
pub fn verify_publisher(path: &Path, allowed: &[&str]) -> Result<String> {
    match inspect_signature(path)? {
        Signature::Valid(publisher)
            if allowed.is_empty() || allowed.contains(&publisher.as_str()) =>
        {
            Ok(publisher)
        }
        other => Err(fail(format!(
            "Assinatura/editor não validado: {other:?}. Operação bloqueada."
        ))),
    }
}
#[cfg(windows)]
pub fn inspect_signature(path: &Path) -> Result<Signature> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Security::WinTrust::*;
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: all pointers remain alive for both WinVerifyTrust calls; state is closed.
    unsafe {
        let mut file: WINTRUST_FILE_INFO = std::mem::zeroed();
        file.cbStruct = std::mem::size_of_val(&file) as u32;
        file.pcwszFilePath = wide.as_ptr();
        let mut data: WINTRUST_DATA = std::mem::zeroed();
        data.cbStruct = std::mem::size_of_val(&data) as u32;
        data.dwUIChoice = WTD_UI_NONE;
        data.fdwRevocationChecks = WTD_REVOKE_WHOLECHAIN;
        data.dwUnionChoice = WTD_CHOICE_FILE;
        data.Anonymous.pFile = &mut file;
        data.dwStateAction = WTD_STATEACTION_VERIFY;
        let mut action = WINTRUST_ACTION_GENERIC_VERIFY_V2;
        let status = WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            &mut data as *mut _ as *mut _,
        );
        let publisher = if status == 0 {
            use windows_sys::Win32::Security::Cryptography::{
                CERT_NAME_SIMPLE_DISPLAY_TYPE, CertGetNameStringW,
            };
            let provider = WTHelperProvDataFromStateData(data.hWVTStateData);
            let signer = if provider.is_null() {
                std::ptr::null_mut()
            } else {
                WTHelperGetProvSignerFromChain(provider, 0, 0, 0)
            };
            if !signer.is_null() && (*signer).csCertChain > 0 && !(*signer).pasCertChain.is_null() {
                let cert = (*(*signer).pasCertChain).pCert;
                let length = CertGetNameStringW(
                    cert,
                    CERT_NAME_SIMPLE_DISPLAY_TYPE,
                    0,
                    std::ptr::null(),
                    std::ptr::null_mut(),
                    0,
                );
                if length > 1 && length < 4096 {
                    let mut text = vec![0u16; length as usize];
                    CertGetNameStringW(
                        cert,
                        CERT_NAME_SIMPLE_DISPLAY_TYPE,
                        0,
                        std::ptr::null(),
                        text.as_mut_ptr(),
                        length,
                    );
                    String::from_utf16_lossy(&text[..text.len() - 1])
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        data.dwStateAction = WTD_STATEACTION_CLOSE;
        WinVerifyTrust(
            std::ptr::null_mut(),
            &mut action,
            &mut data as *mut _ as *mut _,
        );
        Ok(match status as u32 {
            0 => Signature::Valid(publisher),
            0x800B0100 => Signature::Unsigned,
            code => Signature::Unverifiable(code),
        })
    }
}
#[cfg(not(windows))]
pub fn verify_signature(_path: &Path) -> Result<()> {
    Err(fail("Authenticode requer Windows"))
}
#[cfg(not(windows))]
pub fn inspect_signature(_path: &Path) -> Result<Signature> {
    Err(fail("Authenticode requer Windows"))
}

pub fn open_official(uri: &str) -> Result<()> {
    const SETTINGS: &[&str] = &[
        "ms-settings:startupapps",
        "ms-settings:display-advanced",
        "ms-settings:powersleep",
        "ms-settings:gaming-gamemode",
        "ms-settings:storagesense",
        "ms-settings:defaultapps",
    ];
    const HOSTS: &[&str] = &[
        "atlasos.net",
        "docs.atlasos.net",
        "github.com",
        "www.amd.com",
        "www.nvidia.com",
        "www.intel.com",
        "www.riotgames.com",
        "www.leagueoflegends.com",
        "playvalorant.com",
        "support.brave.app",
        "support.mozilla.org",
        "support.google.com",
        "support.microsoft.com",
        "store.steampowered.com",
        "store.epicgames.com",
        "www.7-zip.org",
        "www.videolan.org",
        "code.visualstudio.com",
        "obsproject.com",
        "www.voidtools.com",
        "discord.com",
    ];
    let permitted = SETTINGS.contains(&uri)
        || reqwest::Url::parse(uri).ok().is_some_and(|u| {
            u.scheme() == "https"
                && u.username().is_empty()
                && u.password().is_none()
                && u.port_or_known_default() == Some(443)
                && u.host_str().is_some_and(|h| HOSTS.contains(&h))
        });
    if !permitted {
        return Err(fail("Link fora da lista de origens oficiais"));
    }
    shell_open(uri)
}
pub fn open_amd_downloads(store: &crate::state::Store) -> Result<()> {
    let folder = store.root.join("downloads/amd");
    if !folder.is_dir() {
        return Err(fail(
            "Nenhuma ferramenta AMD foi preparada. Selecione Drivers e execute o plano primeiro.",
        ));
    }
    shell_open(folder.to_str().ok_or_else(|| fail("Caminho inválido"))?)
}
#[cfg(windows)]
fn shell_open(uri: &str) -> Result<()> {
    use windows_sys::Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL};
    let text: Vec<u16> = uri.encode_utf16().chain(Some(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
    // SAFETY: null-terminated strings and no command-line parameters are passed.
    let status = unsafe {
        ShellExecuteW(
            std::ptr::null_mut(),
            verb.as_ptr(),
            text.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            SW_SHOWNORMAL,
        )
    } as isize;
    if status <= 32 {
        return Err(fail(format!("Não foi possível abrir o recurso ({status})")));
    }
    Ok(())
}
#[cfg(not(windows))]
fn shell_open(_uri: &str) -> Result<()> {
    Err(fail("Abertura assistida requer Windows"))
}

pub fn winget_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        struct Apartment(bool);
        impl Drop for Apartment {
            fn drop(&mut self) {
                if self.0 {
                    unsafe {
                        windows::Win32::System::WinRT::RoUninitialize();
                    }
                }
            }
        }
        // SAFETY: apartment lifetime is scoped to this thread; existing STA is accepted.
        let initialized = unsafe {
            windows::Win32::System::WinRT::RoInitialize(
                windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
            )
        };
        if let Err(e) = &initialized
            && e.code().0 as u32 != 0x80010106
        {
            return Err(fail(format!("WinRT indisponível: {e}")));
        }
        let _apartment = Apartment(initialized.is_ok());
        let manager = windows::Management::Deployment::PackageManager::new()
            .map_err(|e| fail(e.to_string()))?;
        let packages = manager
            .FindPackagesByUserSecurityIdPackageFamilyName(
                &windows::core::HSTRING::new(),
                &windows::core::HSTRING::from("Microsoft.DesktopAppInstaller_8wekyb3d8bbwe"),
            )
            .map_err(|e| fail(format!("App Installer indisponível: {e}")))?;
        for package in packages {
            let identity = package.Id().map_err(|e| fail(e.to_string()))?;
            if identity.FamilyName().map_err(|e| fail(e.to_string()))?
                != "Microsoft.DesktopAppInstaller_8wekyb3d8bbwe"
            {
                continue;
            }
            let path = PathBuf::from(
                package
                    .InstalledPath()
                    .map_err(|e| fail(e.to_string()))?
                    .to_string(),
            )
            .join("winget.exe");
            if path.is_file() && verify_signature(&path).is_ok() {
                return Ok(path);
            }
        }
        Err(fail(
            "WinGet assinado não encontrado em WindowsApps. Instale/atualize o App Installer pela Microsoft Store.",
        ))
    }
    #[cfg(not(windows))]
    {
        Err(fail("WinGet requer Windows"))
    }
}
