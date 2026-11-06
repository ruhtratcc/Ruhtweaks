use crate::{Result, fail, system_info::SystemInfo};
use serde::{Deserialize, Serialize};
use std::io::Read;
pub mod legacy;
pub mod preflight;
pub mod verification;
pub const REVIEWED_TAG: &str = "0.5.0-hotfix";
pub const REVIEWED_DIGEST: &str =
    "sha256:13c4d8e7fc0ed38c37c5942ebcabf7f636389972e6de391a192ae649967602d8";
pub const GUIDE: &str = "https://docs.atlasos.net/getting-started/install/install-playbook/";
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub name: String,
    pub browser_download_url: String,
    pub digest: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub tag_name: String,
    pub html_url: String,
    pub assets: Vec<Asset>,
}
pub fn release(repository: &str) -> Result<Release> {
    release_with(&crate::security::client()?, repository)
}
pub fn release_with(client: &reqwest::blocking::Client, repository: &str) -> Result<Release> {
    if ![
        "Atlas-OS/Atlas",
        "Ak4ai/Litecord",
        "Ameliorated-LLC/trusted-uninstaller-cli",
    ]
    .contains(&repository)
    {
        return Err(fail("Repositório não autorizado"));
    }
    let response = client
        .get(format!(
            "https://api.github.com/repos/{repository}/releases/latest"
        ))
        .send()?
        .error_for_status()?;
    let mut bytes = Vec::new();
    response.take(4 * 1024 * 1024 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > 4 * 1024 * 1024 {
        return Err(fail("Metadados excedem limite"));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
pub fn compatibility(info: &SystemInfo) -> std::result::Result<(), String> {
    if !info.architecture.starts_with("x86_64") {
        return Err(
            "Este adaptador validado cobre apenas x86_64; ARM64 requer validação própria.".into(),
        );
    }
    if ![26100, 26200].contains(&info.build) {
        return Err("Release revisada: Windows 11 24H2/25H2 (builds 26100/26200). Windows 10 e outras builds estão bloqueadas.".into());
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
        return Err("Edição não validada; Windows Home não é suportado.".into());
    }
    Ok(())
}
pub fn description(info: &SystemInfo) -> String {
    let status = match compatibility(info) {
        Ok(()) => "Compatível e pronto para preparação de arquivos.".into(),
        Err(reason) => format!("Incompatível: {reason}"),
    };
    format!(
        "{status}\nRelease revisada: {REVIEWED_TAG}\n\nO Ruhtweaks prepara AME Beta 0.8.4 e o playbook com hashes oficiais revisados. A abertura exige confirmação específica, backup e leitura do guia de instalação limpa. O AME abre sem argumentos; o usuário escolhe o playbook e suas opções.\n\nDefenderToggled/NoAntivirus/UCPDDisabled são requisitos reais DURANTE a preparação oficial. Poder manter Defender ao final não elimina esses requisitos. O Ruhtweaks não altera esses controles, não suprime avisos e não comanda a aplicação; interrompa o fluxo se forem exigidas mudanças incompatíveis com suas regras.\n\nNão foi validada uma CLI pública para comandar a aplicação completa deste playbook. Os comandos IPC/execução interna do core não são usados como substitutos.\n\nEscolha defender-enable, mitigations-default e auto-updates-default, sem disable-core-isolation. Escolha browser-brave se desejar Brave no AME. Essas opções são orientações; não são injetadas no playbook.\n\nA verificação lê o resultado oficial AME (AtlasOS 0.5.0, horário posterior e ErrorLevel=0), opções seguras, boot posterior, ausência de reboot pendente, Defender, firewall e UCPD ativos. O registro não distingue hotfix nem atesta criptograficamente o pacote aplicado. Ausência de prova mantém a etapa pendente.\n\nNão há formatação, reboot automático ou rollback integral de Atlas. Você pode continuar com apps sem concluir Atlas mediante novo plano aprovado."
    )
}
pub fn verified_playbook(release: &Release) -> Result<&Asset> {
    if release.tag_name != REVIEWED_TAG {
        return Err(fail(format!(
            "Release {} não revisada; esperado {}",
            release.tag_name, REVIEWED_TAG
        )));
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "AtlasPlaybook_v0.5.0-hotfix.apbx")
        .ok_or_else(|| fail("Playbook esperado não foi publicado"))?;
    if asset.digest.as_deref() != Some(REVIEWED_DIGEST) {
        return Err(fail(
            "Artefato Atlas alterado desde a revisão. Download bloqueado até nova validação.",
        ));
    }
    Ok(asset)
}
