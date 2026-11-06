//! Product identities reviewed against the vendor's installer, never executable-name guesses.
use super::Detection;
use crate::{Result, fail};
pub const LITECORD_ID: &str = "{6E4F5D7B-9A3C-4D2E-8B1F-5C7D9A2B4E6F}_is1";
pub fn confirmed_registration(key: &str, detection: Detection) -> Detection {
    if matches!(key, "riot_lol_br" | "riot_valorant_br") && detection == Detection::Present {
        // A registration entry cannot prove authenticated patcher completion or
        // game/component readiness. Do not repeat an already registered bootstrapper.
        Detection::Unknown("Produto Riot já registrado. Conclua login, download e componentes no Riot Client. Registro WinGet não comprova que jogo/Vanguard estejam prontos; o instalador não será repetido.".into())
    } else {
        detection
    }
}

#[cfg(windows)]
pub fn detect(key: &str) -> Result<Detection> {
    use winreg::{RegKey, enums::*};
    if key != "litecord" && key != "riot" {
        return Err(fail("Produto não validado"));
    }
    for hive in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            let root = match RegKey::predef(hive).open_subkey_with_flags(
                "Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
                KEY_READ | view,
            ) {
                Ok(key) => key,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for name in root.enum_keys() {
                let name = name?;
                if key == "litecord" && name != LITECORD_ID {
                    continue;
                }
                let item = root.open_subkey_with_flags(&name, KEY_READ | view)?;
                let display: String = item.get_value("DisplayName").unwrap_or_default();
                let publisher: String = item.get_value("Publisher").unwrap_or_default();
                let matches = if key == "litecord" {
                    display == "Litecord"
                        && publisher == "Litecord Project"
                        && item
                            .get_value::<String, _>("URLInfoAbout")
                            .unwrap_or_default()
                            == "https://github.com/Ak4ai/Litecord"
                } else {
                    display == "Riot Client"
                        && matches!(publisher.as_str(), "Riot Games, Inc" | "Riot Games, Inc.")
                };
                if !matches {
                    continue;
                }
                let location: String = item.get_value("InstallLocation").unwrap_or_default();
                let folder = std::path::Path::new(&location);
                if !folder.is_absolute() || location.starts_with("\\\\") {
                    return Ok(Detection::Unknown(
                        "Registro do produto existe, mas o caminho local não foi confirmado."
                            .into(),
                    ));
                }
                let exe = folder.join(if key == "litecord" {
                    "litecord.exe"
                } else {
                    "RiotClientServices.exe"
                });
                if !exe.is_file() {
                    return Ok(Detection::Unknown(
                        "Produto registrado com executável ausente. Revise a instalação existente."
                            .into(),
                    ));
                }
                if key == "riot" {
                    crate::security::verify_signature(&exe)?;
                }
                if crate::security::verify_pe_header(&exe).is_err() {
                    return Ok(Detection::Unknown("Registro do produto existe, mas o executável está incompleto ou inválido. Revalide/repare pelo instalador oficial.".into()));
                }
                return Ok(Detection::Present);
            }
        }
    }
    Ok(Detection::Absent)
}
#[cfg(not(windows))]
pub fn detect(_key: &str) -> Result<Detection> {
    Err(fail("Detecção de produto requer Windows"))
}
