use crate::{
    Result, fail, security,
    state::{Profile, Status, Store},
    system_info::SystemInfo,
};
use std::sync::atomic::AtomicBool;
pub const TAG: &str = "0.4.1";
pub const SOURCE: &str =
    "https://github.com/Atlas-OS/Atlas/releases/download/0.4.1/AtlasPlaybook_v0.4.1.apbx";
// Calculated from the official release, independently matched with the user's file.
// GitHub's old asset has no published digest. Never inherit the modern adapter.
pub const DIGEST: &str = "sha256:e2fda94d1b340b2847cc8615f2474fc6952b206953920d32ef42e44afeb88680";
pub fn description(info: &SystemInfo) -> String {
    let status =
        compatibility(info).map_or_else(|e| e, |()| "Windows 10 22H2 x64 reconhecido.".into());
    format!(
        "{status} Atlas 0.4.1 é legado e não recomendado. Somente preparação do arquivo oficial verificado. Aplicação assistida; instalação limpa e requisitos de proteção continuam externos. Não desativamos Defender, firewall ou UCPD. O resultado dessa versão não é confirmado automaticamente."
    )
}
pub fn compatibility(info: &SystemInfo) -> std::result::Result<(), String> {
    if !info.architecture.starts_with("x86_64") || info.build != 19045 {
        return Err("Atlas legado exige Windows 10 22H2 x64 (19045).".into());
    }
    if ![
        "Professional",
        "ProfessionalN",
        "Enterprise",
        "EnterpriseN",
        "Education",
        "EducationN",
    ]
    .contains(&info.edition.as_str())
    {
        return Err("Edição não validada para o adaptador legado.".into());
    }
    Ok(())
}
pub fn prepare(
    info: &SystemInfo,
    profile: &Profile,
    store: &Store,
    cancel: &AtomicBool,
    log: &mut dyn FnMut(String),
) -> Result<(Status, String)> {
    if !profile.atlas || !profile.atlas_legacy {
        return Err(fail("Atlas legado não selecionado"));
    }
    compatibility(info).map_err(fail)?;
    if super::preflight::reboot_pending()? {
        return Ok((
            Status::AwaitingReboot,
            "Reinicie o Windows e revalide antes de preparar Atlas.".into(),
        ));
    }
    let destination = store
        .root
        .join("downloads")
        .join(security::validate_digest(DIGEST)?)
        .join("AtlasPlaybook_v0.4.1.apbx");
    let mut last = std::time::Instant::now() - std::time::Duration::from_secs(1);
    security::download_verified(SOURCE, DIGEST, &destination, cancel, |done, total| {
        if last.elapsed() >= std::time::Duration::from_millis(200) || total == Some(done) {
            log(format!("Atlas legado: {done} bytes / {total:?}"));
            last = std::time::Instant::now();
        }
    })?;
    Ok((
        Status::RequiresAction,
        format!(
            "Atlas 0.4.1 preparado: {}. Legado, não recomendado. Hash corresponde à cópia oficial revisada; não é um digest publicado pelo fornecedor. Aplicação assistida: consulte https://github.com/Atlas-OS/Atlas/releases/tag/0.4.1 . Faça backup. O playbook exige instalação limpa e mudanças de proteção; o Ruhtweaks não as executa. Não há prova de aplicação nem conclusão automática para esta versão.",
            destination.display()
        ),
    ))
}
