//! WinGet's documented JSON export avoids parsing localized installed-app tables.
use crate::{
    Result, catalog, fail,
    state::{Profile, Status, UpdateRequest, valid_version},
};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Export {
    sources: Vec<Source>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Source {
    source_details: SourceDetails,
    packages: Vec<Package>,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SourceDetails {
    identifier: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Package {
    package_identifier: String,
    version: Option<String>,
}

pub fn parse_export(json: &str) -> Result<BTreeMap<String, String>> {
    let export: Export = serde_json::from_str(json.trim_start_matches('\u{feff}'))?;
    let mut versions = BTreeMap::new();
    for source in export.sources {
        if source.source_details.identifier != "Microsoft.Winget.Source_8wekyb3d8bbwe" {
            continue;
        }
        for package in source.packages {
            if let Some(version) = package.version.filter(|v| valid_version(v))
                && versions
                    .insert(package.package_identifier, version)
                    .is_some()
            {
                return Err(fail(
                    "Múltiplas instalações do mesmo pacote. Atualização automática bloqueada.",
                ));
            }
        }
    }
    Ok(versions)
}

pub fn parse_versions(text: &str) -> Vec<String> {
    // `show --versions` prints a one-column table sorted by WinGet's own version
    // comparator. Require its separator, never interpret diagnostics as versions.
    let mut table = false;
    let mut versions = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.len() >= 3 && line.bytes().all(|b| b == b'-') {
            table = true;
            continue;
        }
        if table && valid_version(line) {
            versions.push(line.into());
        }
    }
    versions
}

pub fn candidate(key: &str, installed: &str, versions: &[String]) -> Option<UpdateRequest> {
    let target = versions.first()?;
    // An unmapped ARP version or a newer/pre-release installation is not an
    // invitation to downgrade. Only offer versions ordered above the known one.
    let index = versions.iter().position(|v| v == installed)?;
    (index > 0).then(|| UpdateRequest {
        key: key.into(),
        from: installed.into(),
        to: target.clone(),
    })
}

pub fn mode(key: &str, profile: &Profile) -> &'static str {
    // These packages do not enable the application's logon entry by default.
    // For clients with unvalidated startup defaults keep their component dialog.
    let startup_reviewed = matches!(
        key,
        "brave" | "firefox" | "chrome" | "edge" | "7zip" | "vlc" | "vscode" | "obs"
    );
    if profile.prefer_silent && startup_reviewed {
        "--silent"
    } else {
        "--interactive"
    }
}

pub fn command(
    key: &str,
    profile: &Profile,
    update: Option<&UpdateRequest>,
) -> Result<Vec<String>> {
    let id = catalog::find(key)
        .and_then(|app| app.winget)
        .ok_or_else(|| fail("Pacote WinGet não validado"))?;
    let mut args = vec![
        if update.is_some() {
            "upgrade"
        } else {
            "install"
        },
        "--id",
        id,
        "--exact",
        "--source",
        "winget",
        mode(key, profile),
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--disable-interactivity",
    ]
    .into_iter()
    .map(str::to_string)
    .collect::<Vec<_>>();
    if let Some(update) = update {
        if update.key != key || !valid_version(&update.to) || !valid_version(&update.from) {
            return Err(fail("Versão não aprovada"));
        }
        args.extend(["--version".into(), update.to.clone()]);
    } else {
        args.push("--no-upgrade".into());
    }
    Ok(args)
}

pub trait UpdateBackend {
    fn installed_version(&mut self, key: &str) -> Result<Option<String>>;
    fn upgrade(&mut self, request: &UpdateRequest, profile: &Profile) -> Result<i32>;
}
pub fn apply(
    backend: &mut impl UpdateBackend,
    request: &UpdateRequest,
    profile: &Profile,
) -> Result<(Status, String)> {
    profile.validate()?;
    if !profile.accept_terms || !profile.updates.contains(request) {
        return Err(fail("Atualização não autorizada no perfil"));
    }
    let before = backend.installed_version(&request.key)?;
    if before.as_deref() == Some(&request.to) {
        return Ok((
            Status::Completed,
            format!(
                "Versão {} já instalada; atualização não repetida.",
                request.to
            ),
        ));
    }
    if before.as_deref() != Some(&request.from) {
        return Ok((
            Status::RequiresAction,
            "A versão instalada mudou ou não foi identificada. Consulte e aprove um novo plano."
                .into(),
        ));
    }
    let code = backend.upgrade(request, profile)?;
    if code == 3010 {
        return Ok((Status::RequiresAction, "Reinicialização solicitada. Reinicie quando puder e revalide a versão; nenhuma reinicialização automática.".into()));
    }
    if code != 0 {
        return Err(fail(format!(
            "Atualização WinGet falhou (0x{:08X}).",
            code as u32
        )));
    }
    if backend.installed_version(&request.key)?.as_deref() != Some(&request.to) {
        return Ok((
            Status::RequiresAction,
            format!(
                "Processo terminou, mas a versão {} não foi confirmada. Revalide antes de repetir.",
                request.to
            ),
        ));
    }
    Ok((
        Status::Completed,
        format!("Atualização verificada: {} → {}.", request.from, request.to),
    ))
}
