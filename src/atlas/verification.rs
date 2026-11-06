use crate::{Result, fail, state::Status};
#[derive(Clone, Debug)]
pub struct Evidence {
    pub version: String,
    pub name: String,
    pub error_level: u32,
    pub selected_options: Vec<String>,
    pub applied_ms: u64,
}
pub fn evaluate(
    evidence: Option<&Evidence>,
    started_ms: u64,
    rebooted: bool,
    reboot_pending: bool,
    protections_active: bool,
) -> (Status, String) {
    let Some(e) = evidence else {
        return (Status::AwaitingVerification, "AME ainda não publicou um resultado Atlas verificável. Abrir ou fechar o assistente não confirma aplicação.".into());
    };
    if e.version != "0.5.0"
        || e.name != "AtlasOS"
        || e.applied_ms < started_ms
        || e.error_level != 0
    {
        return (Status::AwaitingVerification, "Registro oficial ausente, antigo, de outra versão ou com erros. Consulte o relatório AME; a aplicação não foi confirmada.".into());
    }
    if ![
        "defender-enable",
        "mitigations-default",
        "auto-updates-default",
    ]
    .iter()
    .all(|option| e.selected_options.iter().any(|v| v == option))
    {
        return (Status::AwaitingVerification, "As opções de proteção/atualizações do registro AME não correspondem às opções seguras revisadas. Nenhuma correção será forçada.".into());
    }
    if !rebooted || reboot_pending {
        return (Status::AwaitingReboot, "AME publicou resultado sem erros, mas a reinicialização posterior e o estado do Windows ainda precisam ser confirmados. Reinicie somente quando desejar.".into());
    }
    if [
        "defender-disable",
        "mitigations-disable",
        "auto-updates-disable",
        "disable-core-isolation",
    ]
    .iter()
    .any(|option| e.selected_options.iter().any(|v| v == option))
    {
        return (
            Status::AwaitingVerification,
            "Registro AME contém opções que reduzem proteções. Conclusão automática bloqueada."
                .into(),
        );
    }
    if !protections_active {
        return (Status::AwaitingVerification, "Defender em tempo real, firewall e UCPD não foram confirmados como ativos. Confira Segurança do Windows. Não modificaremos nem ignoraremos proteções.".into());
    }
    (Status::Completed, "AME registrou AtlasOS 0.5.0 sem erros após o início desta etapa. Reinício posterior, opções seguras, Defender, firewall e UCPD confirmados. O registro não distingue o hotfix nem é uma atestação criptográfica do playbook aplicado.".into())
}
pub fn decode_dotnet_utc(value: u64) -> Result<u64> {
    let ticks = value & 0x3fff_ffff_ffff_ffff;
    ticks
        .checked_sub(621_355_968_000_000_000)
        .map(|v| v / 10_000)
        .ok_or_else(|| fail("Data do resultado AME inválida"))
}
#[cfg(windows)]
pub fn read() -> Result<Option<Evidence>> {
    use winreg::{RegKey, enums::*};
    let key = match RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey_with_flags(
        "SOFTWARE\\AME\\Playbooks\\Applied\\{00000000-0000-4000-6174-6C6173203A33}",
        KEY_READ | KEY_WOW64_64KEY,
    ) {
        Ok(k) => k,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    Ok(Some(Evidence {
        version: key.get_value("Version")?,
        name: key.get_value("Name")?,
        error_level: key.get_value("ErrorLevel")?,
        selected_options: key.get_value("SelectedOptions")?,
        applied_ms: decode_dotnet_utc(key.get_value("AppliedTimeUTC")?)?,
    }))
}
#[cfg(not(windows))]
pub fn read() -> Result<Option<Evidence>> {
    Err(fail("Resultado AME requer Windows"))
}
