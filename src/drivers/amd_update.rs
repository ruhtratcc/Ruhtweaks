//! Only assigned AMD Display updates matching enumerated hardware IDs are eligible.
use crate::{
    Result, fail,
    state::{Profile, Status, Store, atomic_json, read_json},
    system_info::SystemInfo,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
#[cfg(windows)]
mod native;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub id: String,
    pub revision: i32,
    pub title: String,
    pub hardware_id: String,
    pub class: String,
    pub manufacturer: String,
    pub requires_ui: bool,
}
#[derive(Clone, Debug)]
pub struct Device {
    pub instance: String,
    pub ids: Vec<String>,
}
pub fn select(candidates: &[Candidate], devices: &[Device]) -> Result<Vec<Candidate>> {
    let mut selected = Vec::new();
    for device in devices {
        let mut matches = candidates
            .iter()
            .filter(|c| {
                let maker = c.manufacturer.to_ascii_lowercase();
                c.class.eq_ignore_ascii_case("Display")
                    && !c.requires_ui
                    && (maker.contains("advanced micro devices") || maker == "amd")
                    && c.hardware_id
                        .to_ascii_uppercase()
                        .starts_with("PCI\\VEN_1002&")
                    && device
                        .ids
                        .iter()
                        .any(|id| id.eq_ignore_ascii_case(&c.hardware_id))
                    && valid_id(&c.id)
                    && c.revision >= 0
            })
            .collect::<Vec<_>>();
        matches.sort_by(|a, b| a.id.cmp(&b.id).then(a.revision.cmp(&b.revision)));
        matches.dedup_by(|a, b| a.id == b.id && a.revision == b.revision);
        if matches.len() > 1 {
            return Err(fail(
                "Windows Update ofereceu drivers AMD ambíguos; seleção automática interrompida.",
            ));
        }
        if let Some(candidate) = matches.first()
            && !selected
                .iter()
                .any(|old: &Candidate| old.id == candidate.id && old.revision == candidate.revision)
        {
            selected.push((*candidate).clone());
        }
    }
    Ok(selected)
}
pub fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                b == b'-'
            } else {
                b.is_ascii_hexdigit()
            }
        })
}
pub trait Backend {
    fn search(&mut self) -> Result<Vec<Candidate>>;
    fn install(&mut self, updates: &[Candidate]) -> Result<u32>;
    fn installed(&mut self, updates: &[Candidate]) -> Result<bool>;
}
pub fn run_with(
    api: &mut impl Backend,
    devices: &[Device],
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Status, String),
) -> Result<(Status, String)> {
    if cancel.load(Ordering::Relaxed) {
        return Ok((
            Status::Cancelled,
            "Cancelado antes de buscar drivers.".into(),
        ));
    }
    report(
        Status::Verifying,
        "Buscando driver AMD compatível no Windows Update…".into(),
    );
    let candidates = api.search()?;
    let selected = select(&candidates, devices)?;
    if selected.is_empty() {
        return Ok((Status::RequiresAction, "Windows Update não ofereceu um driver AMD recomendado compatível. Use a ferramenta oficial AMD; nenhum pacote foi adivinhado.".into()));
    }
    if cancel.load(Ordering::Relaxed) {
        return Ok((
            Status::Cancelled,
            "Cancelado antes de instalar drivers.".into(),
        ));
    }
    report(
        Status::Installing,
        format!(
            "Driver AMD: {}. Confirme o UAC se aparecer. Download e instalação serão feitos pelo Windows Update.",
            selected
                .iter()
                .map(|c| c.title.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    );
    match api.install(&selected)? {
        1223 => Ok((Status::Cancelled, "Instalação AMD cancelada no UAC.".into())),
        3010 => Ok((Status::AwaitingReboot, "Windows Update solicitou reinício. Reinicie quando desejar e revalide os drivers.".into())),
        0 if api.installed(&selected)? => Ok((Status::Completed, "Pacote(s) AMD selecionado(s) confirmado(s) como instalados pelo Windows Update. O Ruhtweaks não solicitou o instalador Adrenalin nem pacotes de chipset.".into())),
        0 => Ok((Status::AwaitingVerification, "Windows Update terminou, mas o resultado ainda não foi confirmado. Revalide antes de repetir.".into())),
        code => Err(fail(format!("Windows Update não concluiu o driver AMD (0x{code:08X}). Nenhuma proteção ou política será alterada."))),
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    schema_version: u32,
    profile_hash: String,
    created_ms: u64,
    parent_pid: u32,
    binary_hash: String,
    boot_before: String,
    updates: Vec<Candidate>,
    needs_reboot: bool,
    other_gpus: bool,
}
fn now_ms() -> Result<u64> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| fail(e.to_string()))?
        .as_millis() as u64)
}
fn validate_request(request: &Request, profile: &Profile) -> Result<()> {
    if request.schema_version != 1
        || !profile.drivers
        || !profile.accept_terms
        || request.profile_hash != profile.fingerprint()?
        || request.updates.is_empty()
        || request.updates.len() > 16
        || now_ms()?.saturating_sub(request.created_ms) > 600_000
        || request.created_ms > now_ms()?
        || request
            .updates
            .iter()
            .any(|c| !valid_id(&c.id) || c.revision < 0)
    {
        return Err(fail("Solicitação AMD inválida, vencida ou não aprovada."));
    }
    Ok(())
}
#[cfg(windows)]
pub fn prepare(
    info: &SystemInfo,
    profile: &Profile,
    store: &Store,
    cancel: &AtomicBool,
    report: &mut dyn FnMut(Status, String),
) -> Result<(Status, String)> {
    let devices = native::devices(info)?;
    if devices.is_empty() {
        return Ok((
            Status::RequiresAction,
            "Nenhuma GPU AMD validada. Use o suporte oficial da GPU detectada.".into(),
        ));
    }
    if crate::atlas::preflight::reboot_pending()? {
        return Ok((
            Status::AwaitingReboot,
            "Reinicie o Windows antes de atualizar o driver AMD.".into(),
        ));
    }
    let mut api = native::Native {
        store,
        profile,
        other_gpus: info
            .gpus
            .iter()
            .any(|g| !g.hardware_id.to_ascii_uppercase().contains("VEN_1002")),
    };
    let mut outcome = run_with(&mut api, &devices, cancel, report)?;
    if outcome.0 == Status::Completed
        && info
            .gpus
            .iter()
            .any(|g| !g.hardware_id.to_ascii_uppercase().contains("VEN_1002"))
    {
        outcome = (
            Status::RequiresAction,
            format!("{} Outras GPUs: use o suporte oficial indicado.", outcome.1),
        );
    }
    Ok(outcome)
}
#[cfg(not(windows))]
pub fn prepare(
    _: &SystemInfo,
    _: &Profile,
    _: &Store,
    _: &AtomicBool,
    _: &mut dyn FnMut(Status, String),
) -> Result<(Status, String)> {
    Err(fail("Requer Windows"))
}
#[cfg(windows)]
pub fn helper(path: &Path, digest: &str) -> Result<u32> {
    // The elevated helper only READS the user's request. It never writes to a
    // user-controlled path with admin rights; its only mutation is scoped WUA.
    if crate::security::digest_file(path)? != crate::security::validate_digest(digest)? {
        return Err(fail("Solicitação AMD alterada"));
    }
    let request: Request = read_json(path)?;
    let root = path
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| fail("Solicitação sem diretório"))?;
    if path
        .parent()
        .and_then(Path::file_name)
        .is_none_or(|n| n != "amd-jobs")
    {
        return Err(fail("Diretório de solicitação inválido"));
    }
    let profile = crate::state::decode_profile(read_json(&root.join("profile.json"))?)?;
    validate_request(&request, &profile)?;
    let journal: crate::state::Journal = read_json(&root.join("journal.json"))?;
    let canonical = crate::installer::plan(&profile, &SystemInfo::default())?;
    if journal.schema_version != crate::state::SCHEMA
        || journal.profile_hash != request.profile_hash
        || !journal.tasks.iter().any(|t| {
            t.action == crate::state::Action::Drivers
                && matches!(
                    t.status,
                    Status::Installing | Status::Preparing | Status::Verifying
                )
        })
        || journal
            .tasks
            .iter()
            .map(|t| (&t.id, &t.action))
            .collect::<Vec<_>>()
            != canonical
                .tasks
                .iter()
                .map(|t| (&t.id, &t.action))
                .collect::<Vec<_>>()
    {
        return Err(fail("Fila AMD não corresponde ao plano aprovado"));
    }
    native::validate_parent(request.parent_pid, &request.binary_hash)?;
    let info = crate::system_info::collect();
    if !info.supported() {
        return Err(fail("Windows não validado"));
    }
    let devices = native::devices(&info)?;
    let selected = select(&request.updates, &devices)?;
    if selected != request.updates {
        return Err(fail("Hardware AMD mudou após aprovação"));
    }
    native::install(&selected)
}
#[cfg(not(windows))]
pub fn helper(_: &Path, _: &str) -> Result<u32> {
    Err(fail("Requer Windows"))
}
#[cfg(windows)]
pub fn revalidate(store: &Store, profile_hash: &str) -> Result<Option<(Status, String)>> {
    let path = store.root.join("amd-pending.json");
    if !path.exists() {
        return Ok(None);
    }
    let request: Request = read_json(&path)?;
    if request.schema_version != 1
        || request.profile_hash != profile_hash
        || request.updates.is_empty()
        || request.updates.len() > 16
    {
        return Ok(None);
    }
    if !native::installed(&request.updates)? {
        return Ok(Some((
            Status::AwaitingVerification,
            "Driver AMD ainda não confirmado pelo Windows Update.".into(),
        )));
    }
    let devices = native::devices(&crate::system_info::collect())?;
    if select(&request.updates, &devices)? != request.updates {
        return Ok(Some((
            Status::RequiresAction,
            "O hardware mudou. Revise os drivers antes de continuar.".into(),
        )));
    }
    if crate::atlas::preflight::reboot_pending()?
        || (request.needs_reboot && request.boot_before == crate::atlas::preflight::boot_id()?)
    {
        return Ok(Some((
            Status::AwaitingReboot,
            "Reinício pendente para concluir o driver AMD.".into(),
        )));
    }
    Ok(Some((if request.other_gpus { Status::RequiresAction } else { Status::Completed }, "Drivers AMD confirmados pelo Windows Update após revalidação. Outras GPUs exigem revisão separada.".into())))
}
#[cfg(not(windows))]
pub fn revalidate(_: &Store, _: &str) -> Result<Option<(Status, String)>> {
    Ok(None)
}
